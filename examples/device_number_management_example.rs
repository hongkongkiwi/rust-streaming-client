use anyhow::Result;
use patrol_client::{
    api::{ApiClient, AddPlivoNumberRequest, AllocateNumberRequest, AddToWhitelistRequest},
    config::Config,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::init();

    // Load configuration (you'd typically load this from a file)
    let config = Config {
        server_url: "https://your-app.convex.site".to_string(),
        auth_token: Some("your-auth-token".to_string()),
        api_key: Some("your-api-key".to_string()),
        network: patrol_client::config::NetworkConfig {
            timeout: 30,
            retry_attempts: 3,
        },
        // ... other config fields
    };

    let api_client = ApiClient::new(config);

    println!("=== Tenant Admin: Plivo Number Management ===");

    // Example 1: Add a new Plivo number to the tenant
    let add_number_request = AddPlivoNumberRequest {
        phone_number: "+1555123456".to_string(),
        number_type: "local".to_string(),
        country: "US".to_string(),
        region: Some("CA".to_string()),
        plivo_number_id: "plivo-abc123".to_string(),
        plivo_auth_id: "MAXXXXXXXXXXXXXXXXXX".to_string(),
        plivo_auth_token: "your-plivo-auth-token".to_string(),
        sms_enabled: true,
        voice_enabled: true,
        whitelist_mode: "enabled".to_string(),
        monthly_cost: Some(1.50),
        setup_cost: Some(0.0),
        currency: Some("USD".to_string()),
        notes: Some("Main communication number for security devices".to_string()),
    };

    match api_client.add_plivo_number(add_number_request).await {
        Ok(number_id) => {
            println!("✅ Added Plivo number successfully! ID: {}", number_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to add Plivo number: {}", e);
        }
    }

    // Example 2: View all tenant Plivo numbers
    match api_client.get_tenant_plivo_numbers(None, None).await {
        Ok(numbers) => {
            println!("\n📞 Tenant Plivo Numbers ({} total):", numbers.len());
            for number in &numbers {
                println!(
                    "  - {} ({}): SMS={}, Voice={}, Allocated={}",
                    number.phone_number,
                    number.number_type,
                    number.sms_enabled,
                    number.voice_enabled,
                    number.is_allocated
                );
                if let Some(device_id) = &number.allocated_to_device_id {
                    println!("    📱 Allocated to device: {}", device_id);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get Plivo numbers: {}", e);
        }
    }

    println!("\n=== Device Number Allocation ===");

    // Example 3: Allocate a number to a specific device
    let device_id = "device-123";
    
    if let Ok(numbers) = api_client.get_tenant_plivo_numbers(Some(false), Some(true)).await {
        if let Some(unallocated_number) = numbers.first() {
            let allocation_request = AllocateNumberRequest {
                plivo_number_id: unallocated_number.id.clone(),
                device_id: device_id.to_string(),
                sms_enabled: true,
                voice_enabled: true,
                daily_sms_limit: Some(100),
                daily_call_limit: Some(20),
                monthly_usage_limit: Some(50.0),
                emergency_bypass_limits: Some(true),
                emergency_contacts_only: Some(false),
            };

            match api_client.allocate_number_to_device(allocation_request).await {
                Ok(_) => {
                    println!(
                        "✅ Allocated number {} to device {}",
                        unallocated_number.phone_number, device_id
                    );
                }
                Err(e) => {
                    eprintln!("❌ Failed to allocate number: {}", e);
                }
            }
        }
    }

    // Example 4: Check device communication capabilities
    match api_client.get_device_communication_capabilities(device_id).await {
        Ok(Some(capabilities)) => {
            println!("\n📱 Device {} Communication Status:", device_id);
            println!("  SMS Enabled: {}", capabilities.sms_enabled);
            println!("  Voice Enabled: {}", capabilities.voice_enabled);
            println!("  Current Month SMS: {}", capabilities.current_month_sms);
            println!("  Current Month Calls: {}", capabilities.current_month_calls);
            println!("  Current Month Cost: ${:.2}", capabilities.current_month_cost);
            
            if let Some(daily_sms) = capabilities.daily_sms_limit {
                println!("  Daily SMS Limit: {}", daily_sms);
            }
            if let Some(daily_calls) = capabilities.daily_call_limit {
                println!("  Daily Call Limit: {}", daily_calls);
            }
            if let Some(monthly_limit) = capabilities.monthly_usage_limit {
                println!("  Monthly Usage Limit: ${:.2}", monthly_limit);
            }
        }
        Ok(None) => {
            println!("📱 Device {} has no communication capabilities configured", device_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to get device capabilities: {}", e);
        }
    }

    // Example 5: Get device's allocated number details
    match api_client.get_device_allocated_number(device_id).await {
        Ok(Some((plivo_number, _capabilities))) => {
            println!("\n📞 Device {} Allocated Number:", device_id);
            println!("  Phone Number: {}", plivo_number.phone_number);
            println!("  Number Type: {}", plivo_number.number_type);
            println!("  Country: {}", plivo_number.country);
            println!("  Whitelist Mode: {}", plivo_number.whitelist_mode);
            if let Some(cost) = plivo_number.monthly_cost {
                println!("  Monthly Cost: ${:.2}", cost);
            }
        }
        Ok(None) => {
            println!("📱 Device {} has no number allocated", device_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to get device number: {}", e);
        }
    }

    println!("\n=== Whitelist Management ===");

    // Example 6: Add emergency services to whitelist
    if let Ok(Some((plivo_number, _))) = api_client.get_device_allocated_number(device_id).await {
        let emergency_whitelist = vec![
            ("911", "Emergency services"),
            ("+1800123456", "Company security dispatch"),
            ("+1555987654", "Site supervisor"),
        ];

        for (number, description) in emergency_whitelist {
            let whitelist_request = AddToWhitelistRequest {
                plivo_number_id: plivo_number.id.clone(),
                allowed_number: number.to_string(),
                number_type: "emergency".to_string(),
                sms_allowed: true,
                voice_allowed: true,
                contact_id: None,
                description: Some(description.to_string()),
            };

            match api_client.add_to_whitelist(whitelist_request).await {
                Ok(whitelist_id) => {
                    println!("✅ Added {} to whitelist: {}", number, whitelist_id);
                }
                Err(e) => {
                    eprintln!("❌ Failed to add {} to whitelist: {}", number, e);
                }
            }
        }

        // Example 7: View current whitelist
        match api_client.get_number_whitelist(&plivo_number.id).await {
            Ok(whitelist) => {
                println!("\n📋 Whitelist for {} ({} entries):", plivo_number.phone_number, whitelist.len());
                for entry in whitelist {
                    println!(
                        "  - {} ({}): SMS={}, Voice={}, Type={}",
                        entry.allowed_number,
                        entry.description.unwrap_or("No description".to_string()),
                        entry.sms_allowed,
                        entry.voice_allowed,
                        entry.number_type
                    );
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to get whitelist: {}", e);
            }
        }
    }

    println!("\n=== Device Communication Testing ===");

    // Example 8: Check if device can send SMS/make calls
    match api_client.can_device_send_sms(device_id).await {
        Ok(can_sms) => println!("📱 Device {} can send SMS: {}", device_id, can_sms),
        Err(e) => eprintln!("❌ Failed to check SMS capability: {}", e),
    }

    match api_client.can_device_make_calls(device_id).await {
        Ok(can_call) => println!("📱 Device {} can make calls: {}", device_id, can_call),
        Err(e) => eprintln!("❌ Failed to check call capability: {}", e),
    }

    // Example 9: Get device usage statistics
    match api_client.get_device_usage_stats(device_id).await {
        Ok(Some((sms_count, call_count, cost))) => {
            println!("\n📊 Device {} Usage This Month:", device_id);
            println!("  SMS Messages: {}", sms_count);
            println!("  Voice Calls: {}", call_count);
            println!("  Total Cost: ${:.2}", cost);
        }
        Ok(None) => {
            println!("📊 No usage statistics available for device {}", device_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to get usage statistics: {}", e);
        }
    }

    println!("\n=== Device-Specific Communication (Required Device ID) ===");

    // Example 10: Send SMS using device-specific number
    if api_client.can_device_send_sms(device_id).await.unwrap_or(false) {
        match api_client
            .send_sms(
                "+1555999888",  // to (must be in whitelist if enabled)
                "Test message from PatrolSight device",
                device_id,      // deviceId is now required
                None,           // incident_id
                Some("medium"), // priority
                Some(false),    // emergency
            )
            .await
        {
            Ok(response) => {
                println!(
                    "✅ SMS sent from device-specific number! Message UUID: {}",
                    response.message_uuid
                );
                println!("   From: {}", response.from_number.unwrap_or("Unknown".to_string()));
            }
            Err(e) => {
                eprintln!("❌ Failed to send SMS: {}", e);
            }
        }
    } else {
        println!("📱 Device {} cannot send SMS (not configured or no number allocated)", device_id);
    }

    // Example 11: Make call using device-specific number
    if api_client.can_device_make_calls(device_id).await.unwrap_or(false) {
        match api_client
            .make_call(
                "+1555999888",  // to (must be in whitelist if enabled)
                device_id,      // deviceId is now required
                None,           // incident_id
                Some("high"),   // priority
                Some(false),    // emergency
                Some(true),     // recording
            )
            .await
        {
            Ok(response) => {
                println!(
                    "✅ Call initiated from device-specific number! Call UUID: {}",
                    response.call_uuid
                );
                println!("   From: {}", response.from_number.unwrap_or("Unknown".to_string()));
            }
            Err(e) => {
                eprintln!("❌ Failed to make call: {}", e);
            }
        }
    } else {
        println!("📱 Device {} cannot make calls (not configured or no number allocated)", device_id);
    }

    println!("\n=== Communication Management Summary ===");
    println!("✅ Device-specific Plivo numbers allocated per device");
    println!("✅ Tenant-level number pool management");
    println!("✅ Admin controls for SMS/Voice capabilities");
    println!("✅ Whitelist functionality for allowed destinations");
    println!("✅ Usage tracking and limits");
    println!("✅ Emergency bypass capabilities");
    println!("✅ Automatic device capability detection");

    Ok(())
}