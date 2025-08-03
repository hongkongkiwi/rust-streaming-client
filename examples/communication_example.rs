use anyhow::Result;
use patrol_client::{
    api::{ApiClient, HardwareInfo},
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

    println!("=== SMS Communication Examples ===");

    // Example 1: Send a regular SMS
    match api_client
        .send_sms(
            "+1234567890",           // to
            "Hello from PatrolSight!", // message
            Some("device-123"),      // device_id
            None,                    // incident_id
            Some("medium"),          // priority
            Some(false),             // emergency
        )
        .await
    {
        Ok(response) => {
            println!(
                "✅ SMS sent successfully! Message UUID: {}",
                response.message_uuid
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to send SMS: {}", e);
        }
    }

    // Example 2: Send an emergency SMS
    match api_client
        .send_emergency_sms(
            "+1234567890",
            "EMERGENCY: Security breach detected at Site A",
            Some("device-123"),
            Some("incident-456"),
        )
        .await
    {
        Ok(response) => {
            println!(
                "🚨 Emergency SMS sent! Message UUID: {}",
                response.message_uuid
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to send emergency SMS: {}", e);
        }
    }

    println!("\n=== Voice Call Examples ===");

    // Example 3: Make a regular call
    match api_client
        .make_call(
            "+1234567890",      // to
            Some("device-123"), // device_id
            None,               // incident_id
            Some("high"),       // priority
            Some(false),        // emergency
            Some(true),         // recording
        )
        .await
    {
        Ok(response) => {
            println!("📞 Call initiated! Call UUID: {}", response.call_uuid);
        }
        Err(e) => {
            eprintln!("❌ Failed to make call: {}", e);
        }
    }

    // Example 4: Make an emergency call
    match api_client
        .make_emergency_call(
            "+1234567890",
            Some("device-123"),
            Some("incident-456"),
        )
        .await
    {
        Ok(response) => {
            println!("🚨 Emergency call initiated! Call UUID: {}", response.call_uuid);
        }
        Err(e) => {
            eprintln!("❌ Failed to make emergency call: {}", e);
        }
    }

    println!("\n=== Communication History Examples ===");

    // Example 5: Get SMS history
    match api_client
        .get_sms_history(Some("device-123"), None, Some(10))
        .await
    {
        Ok(sms_history) => {
            println!("📱 SMS History ({} messages):", sms_history.len());
            for sms in sms_history.iter().take(3) {
                println!(
                    "  - To: {}, Status: {}, Text: {}",
                    sms.to, sms.status, sms.text
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get SMS history: {}", e);
        }
    }

    // Example 6: Get call history
    match api_client
        .get_call_history(Some("device-123"), None, Some(10))
        .await
    {
        Ok(call_history) => {
            println!("📞 Call History ({} calls):", call_history.len());
            for call in call_history.iter().take(3) {
                println!(
                    "  - To: {}, Status: {}, Duration: {:?}s",
                    call.to, call.status, call.duration
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get call history: {}", e);
        }
    }

    println!("\n=== Contact Management Examples ===");

    // Example 7: Get emergency contacts
    match api_client.get_contacts(Some("emergency"), None).await {
        Ok(contacts) => {
            println!("🚨 Emergency Contacts ({} contacts):", contacts.len());
            for contact in contacts.iter().take(3) {
                println!(
                    "  - {}: {} (SMS: {}, Calls: {})",
                    contact.name,
                    contact.phone_number,
                    contact.can_receive_sms,
                    contact.can_receive_calls
                );
            }

            // Example 8: Send broadcast SMS to emergency contacts
            if !contacts.is_empty() {
                match api_client
                    .send_broadcast_sms(
                        &contacts,
                        "System Alert: All systems operational",
                        Some("device-123"),
                        None,
                        Some("medium"),
                    )
                    .await
                {
                    Ok(results) => {
                        let success_count = results.iter().filter(|r| r.is_ok()).count();
                        println!(
                            "📡 Broadcast SMS sent to {}/{} contacts",
                            success_count,
                            results.len()
                        );
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to send broadcast SMS: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get emergency contacts: {}", e);
        }
    }

    println!("\n=== Communication System Ready ===");
    println!("The PatrolSight device can now:");
    println!("✅ Send SMS messages via Plivo");
    println!("✅ Make voice calls to landlines via Plivo");
    println!("✅ Track communication history");
    println!("✅ Manage emergency contacts");
    println!("✅ Send broadcast messages");
    println!("✅ Handle emergency communications");

    Ok(())
}