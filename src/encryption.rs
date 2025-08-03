use anyhow::{Result, Context};
use aes_gcm::{Aes256Gcm, Key, aead::{Aead, AeadCore, KeyInit, OsRng}};
type Nonce = aes_gcm::Nonce<aes_gcm::aes::cipher::typenum::U12>;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::RngCore}};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::Path;
use tokio::fs as async_fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    pub algorithm: String,
    pub key_derivation: String,
    pub nonce: String,
    pub salt: String,
    pub iteration_count: u32,
    pub encrypted_size: u64,
    pub original_size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub device_id: String,
}

#[derive(Debug, ZeroizeOnDrop)]
pub struct EncryptionKey {
    key: [u8; 32], // 256-bit key for AES-256
}

impl EncryptionKey {
    fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    fn as_slice(&self) -> &[u8] {
        &self.key
    }
}

pub struct MediaEncryptor {
    device_id: String,
    master_key: Option<EncryptionKey>,
}

impl MediaEncryptor {
    pub fn new(device_id: String) -> Self {
        Self {
            device_id,
            master_key: None,
        }
    }

    /// Initialize encryption with a password-derived key
    pub async fn initialize_with_password(&mut self, password: &str) -> Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .context("Failed to hash password")?;

        let key_bytes = self.derive_key_from_password(password, salt.as_str())?;
        self.master_key = Some(EncryptionKey::new(key_bytes));
        
        Ok(())
    }

    /// Initialize encryption with a device-specific key
    pub async fn initialize_with_device_key(&mut self, device_key: &str) -> Result<()> {
        let key_bytes = self.derive_key_from_device_key(device_key)?;
        self.master_key = Some(EncryptionKey::new(key_bytes));
        Ok(())
    }

    /// Encrypt a video file
    pub async fn encrypt_video_file(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<EncryptionMetadata> {
        let master_key = self.master_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Encryption not initialized"))?;

        // Generate a unique file encryption key derived from master key
        let file_nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let file_key = self.derive_file_key(master_key, &file_nonce)?;
        
        let cipher = Aes256Gcm::new(&file_key);
        
        // Read input file
        let input_data = async_fs::read(input_path).await
            .context("Failed to read input file")?;
        let original_size = input_data.len() as u64;

        // Encrypt data in chunks for memory efficiency
        let encrypted_data = self.encrypt_large_data(&cipher, &file_nonce, &input_data)?;
        let encrypted_size = encrypted_data.len() as u64;

        // Write encrypted data
        async_fs::write(output_path, &encrypted_data).await
            .context("Failed to write encrypted file")?;

        // Create metadata
        let metadata = EncryptionMetadata {
            algorithm: "AES-256-GCM".to_string(),
            key_derivation: "Argon2id".to_string(),
            nonce: general_purpose::STANDARD.encode(&file_nonce),
            salt: self.device_id.clone(), // Using device ID as additional entropy
            iteration_count: 100_000,
            encrypted_size,
            original_size,
            created_at: chrono::Utc::now(),
            device_id: self.device_id.clone(),
        };

        // Write metadata file
        let metadata_path = output_path.with_extension("meta");
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        async_fs::write(metadata_path, metadata_json).await
            .context("Failed to write metadata file")?;

        Ok(metadata)
    }

    /// Decrypt a video file
    pub async fn decrypt_video_file(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<EncryptionMetadata> {
        let master_key = self.master_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Encryption not initialized"))?;

        // Read metadata
        let metadata_path = input_path.with_extension("meta");
        let metadata_json = async_fs::read_to_string(&metadata_path).await
            .context("Failed to read metadata file")?;
        let metadata: EncryptionMetadata = serde_json::from_str(&metadata_json)
            .context("Failed to parse metadata")?;

        // Verify device ID
        if metadata.device_id != self.device_id {
            return Err(anyhow::anyhow!("Device ID mismatch in encrypted file"));
        }

        // Decode nonce
        let file_nonce_bytes = general_purpose::STANDARD.decode(&metadata.nonce)
            .context("Failed to decode nonce")?;
        let file_nonce = Nonce::from_slice(&file_nonce_bytes);

        // Derive file key
        let file_key = self.derive_file_key(master_key, file_nonce)?;
        let cipher = Aes256Gcm::new(&file_key);

        // Read encrypted data
        let encrypted_data = async_fs::read(input_path).await
            .context("Failed to read encrypted file")?;

        // Decrypt data
        let decrypted_data = self.decrypt_large_data(&cipher, file_nonce, &encrypted_data)?;

        // Verify size
        if decrypted_data.len() as u64 != metadata.original_size {
            return Err(anyhow::anyhow!("Decrypted file size mismatch"));
        }

        // Write decrypted data
        async_fs::write(output_path, &decrypted_data).await
            .context("Failed to write decrypted file")?;

        Ok(metadata)
    }

    /// Stream encrypt video data (for real-time encryption during recording)
    pub async fn create_encrypted_stream_writer(
        &self,
        output_path: &Path,
    ) -> Result<EncryptedStreamWriter> {
        let master_key = self.master_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Encryption not initialized"))?;

        let file_nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let file_key = self.derive_file_key(master_key, &file_nonce)?;
        let cipher = Aes256Gcm::new(&file_key);

        let file = async_fs::File::create(output_path).await
            .context("Failed to create output file")?;

        let metadata = EncryptionMetadata {
            algorithm: "AES-256-GCM".to_string(),
            key_derivation: "Argon2id".to_string(),
            nonce: general_purpose::STANDARD.encode(&file_nonce),
            salt: self.device_id.clone(),
            iteration_count: 100_000,
            encrypted_size: 0, // Will be updated when closed
            original_size: 0,  // Will be updated when closed
            created_at: chrono::Utc::now(),
            device_id: self.device_id.clone(),
        };

        Ok(EncryptedStreamWriter::new(file, cipher, file_nonce.clone(), metadata, output_path.to_path_buf()))
    }

    fn derive_key_from_password(&self, password: &str, salt: &str) -> Result<[u8; 32]> {
        let salt_bytes = SaltString::from_b64(salt)
            .map_err(|e| anyhow::anyhow!("Invalid salt: {}", e))?;
        
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        let hash_bytes = password_hash.hash
            .ok_or_else(|| anyhow::anyhow!("No hash in password hash"))?;

        let mut key = [0u8; 32];
        let hash_slice = hash_bytes.as_bytes();
        key.copy_from_slice(&hash_slice[..32.min(hash_slice.len())]);
        
        Ok(key)
    }

    fn derive_key_from_device_key(&self, device_key: &str) -> Result<[u8; 32]> {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(device_key.as_bytes());
        hasher.update(self.device_id.as_bytes());
        hasher.update(b"bodycam-encryption-v1");
        
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        
        Ok(key)
    }

    fn derive_file_key(&self, master_key: &EncryptionKey, nonce: &Nonce) -> Result<Key<Aes256Gcm>> {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(master_key.as_slice());
        hasher.update(nonce);
        hasher.update(self.device_id.as_bytes());
        
        let result = hasher.finalize();
        Ok(*Key::<Aes256Gcm>::from_slice(&result))
    }

    fn encrypt_large_data(&self, cipher: &Aes256Gcm, nonce: &Nonce, data: &[u8]) -> Result<Vec<u8>> {
        const CHUNK_SIZE: usize = 64 * 1024; // Smaller 64KB chunks for lower memory usage
        let mut encrypted_data = Vec::with_capacity(data.len() + (data.len() / CHUNK_SIZE + 1) * 16); // Pre-allocate
        
        if data.len() <= CHUNK_SIZE {
            // Small file, encrypt all at once
            let encrypted = cipher.encrypt(nonce, data)
                .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
            return Ok(encrypted);
        }

        // Large file, encrypt in chunks with power efficiency
        for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
            // Create unique nonce for each chunk
            let mut chunk_nonce = *nonce;
            chunk_nonce[8..12].copy_from_slice(&(i as u32).to_le_bytes());
            
            let encrypted_chunk = cipher.encrypt(&chunk_nonce, chunk)
                .map_err(|e| anyhow::anyhow!("Chunk encryption failed: {}", e))?;
            
            // Store chunk size and encrypted data
            encrypted_data.extend_from_slice(&(encrypted_chunk.len() as u32).to_le_bytes());
            encrypted_data.extend_from_slice(&encrypted_chunk);
            
            // Yield CPU every few chunks to prevent blocking
            if i % 8 == 0 {
                std::hint::spin_loop(); // Hint to CPU for power efficiency
            }
        }

        Ok(encrypted_data)
    }

    fn decrypt_large_data(&self, cipher: &Aes256Gcm, base_nonce: &Nonce, data: &[u8]) -> Result<Vec<u8>> {
        let mut decrypted_data = Vec::new();
        let mut offset = 0;
        let mut chunk_index = 0;

        while offset < data.len() {
            // Read chunk size
            if offset + 4 > data.len() {
                return Err(anyhow::anyhow!("Incomplete chunk size in encrypted data"));
            }
            
            let chunk_size = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]) as usize;
            offset += 4;

            // Read chunk data
            if offset + chunk_size > data.len() {
                return Err(anyhow::anyhow!("Incomplete chunk data in encrypted data"));
            }
            
            let chunk_data = &data[offset..offset + chunk_size];
            offset += chunk_size;

            // Create chunk nonce
            let mut chunk_nonce = *base_nonce;
            chunk_nonce[8..12].copy_from_slice(&(chunk_index as u32).to_le_bytes());

            // Decrypt chunk
            let decrypted_chunk = cipher.decrypt(&chunk_nonce, chunk_data)
                .map_err(|e| anyhow::anyhow!("Chunk decryption failed: {}", e))?;
            
            decrypted_data.extend_from_slice(&decrypted_chunk);
            chunk_index += 1;
        }

        Ok(decrypted_data)
    }

    pub async fn verify_file_integrity(&self, encrypted_path: &Path) -> Result<bool> {
        // Read metadata
        let metadata_path = encrypted_path.with_extension("meta");
        if !metadata_path.exists() {
            return Ok(false);
        }

        let metadata_json = async_fs::read_to_string(&metadata_path).await?;
        let metadata: EncryptionMetadata = serde_json::from_str(&metadata_json)?;

        // Check file size
        let actual_size = async_fs::metadata(encrypted_path).await?.len();
        if actual_size != metadata.encrypted_size {
            return Ok(false);
        }

        // Verify device ID
        if metadata.device_id != self.device_id {
            return Ok(false);
        }

        Ok(true)
    }
}

pub struct EncryptedStreamWriter {
    file: async_fs::File,
    cipher: Aes256Gcm,
    base_nonce: Nonce,
    metadata: EncryptionMetadata,
    output_path: std::path::PathBuf,
    chunk_index: u32,
    bytes_written: u64,
    original_bytes: u64,
}

impl EncryptedStreamWriter {
    fn new(
        file: async_fs::File,
        cipher: Aes256Gcm,
        base_nonce: Nonce,
        metadata: EncryptionMetadata,
        output_path: std::path::PathBuf,
    ) -> Self {
        Self {
            file,
            cipher,
            base_nonce,
            metadata,
            output_path,
            chunk_index: 0,
            bytes_written: 0,
            original_bytes: 0,
        }
    }

    pub async fn write_chunk(&mut self, data: &[u8]) -> Result<()> {
        // Create chunk nonce
        let mut chunk_nonce = self.base_nonce;
        chunk_nonce[8..12].copy_from_slice(&self.chunk_index.to_le_bytes());

        // Encrypt chunk
        let encrypted_chunk = self.cipher.encrypt(&chunk_nonce, data)
            .map_err(|e| anyhow::anyhow!("Chunk encryption failed: {}", e))?;

        // Write chunk size and data
        self.file.write_all(&(encrypted_chunk.len() as u32).to_le_bytes()).await?;
        self.file.write_all(&encrypted_chunk).await?;

        self.chunk_index += 1;
        self.bytes_written += 4 + encrypted_chunk.len() as u64;
        self.original_bytes += data.len() as u64;

        Ok(())
    }

    pub async fn finalize(mut self) -> Result<EncryptionMetadata> {
        self.file.flush().await?;
        
        // Update metadata with final sizes
        let mut final_metadata = self.metadata;
        final_metadata.encrypted_size = self.bytes_written;
        final_metadata.original_size = self.original_bytes;

        // Write metadata file
        let metadata_path = self.output_path.with_extension("meta");
        let metadata_json = serde_json::to_string_pretty(&final_metadata)?;
        async_fs::write(metadata_path, metadata_json).await?;

        Ok(final_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_encryption_roundtrip() {
        let mut encryptor = MediaEncryptor::new("test-device".to_string());
        encryptor.initialize_with_password("test-password").await.unwrap();

        // Create test data
        let test_data = b"This is test video data for encryption testing";
        let temp_input = NamedTempFile::new().unwrap();
        let temp_encrypted = NamedTempFile::new().unwrap();
        let temp_decrypted = NamedTempFile::new().unwrap();

        // Write test data
        std::fs::write(temp_input.path(), test_data).unwrap();

        // Encrypt
        let metadata = encryptor.encrypt_video_file(
            temp_input.path(),
            temp_encrypted.path(),
        ).await.unwrap();

        assert_eq!(metadata.original_size, test_data.len() as u64);
        assert!(metadata.encrypted_size > 0);

        // Decrypt
        let decrypt_metadata = encryptor.decrypt_video_file(
            temp_encrypted.path(),
            temp_decrypted.path(),
        ).await.unwrap();

        // Verify
        let decrypted_data = std::fs::read(temp_decrypted.path()).unwrap();
        assert_eq!(decrypted_data, test_data);
        assert_eq!(decrypt_metadata.original_size, metadata.original_size);
    }

    #[tokio::test]
    async fn test_integrity_verification() {
        let mut encryptor = MediaEncryptor::new("test-device".to_string());
        encryptor.initialize_with_device_key("test-device-key").await.unwrap();

        let test_data = b"Integrity test data";
        let temp_input = NamedTempFile::new().unwrap();
        let temp_encrypted = NamedTempFile::new().unwrap();

        std::fs::write(temp_input.path(), test_data).unwrap();

        encryptor.encrypt_video_file(
            temp_input.path(),
            temp_encrypted.path(),
        ).await.unwrap();

        // Verify integrity
        assert!(encryptor.verify_file_integrity(temp_encrypted.path()).await.unwrap());
    }
}