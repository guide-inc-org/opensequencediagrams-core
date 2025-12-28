use osd_core::{parse, render, Diagram, Item};
use std::fs;

#[test]
fn test_delay_parsing() {
    let input = r#"
participant A
participant B
A->(7)B: Slow message
"#;
    let diagram = parse(input).unwrap();
    // Check that delay is parsed correctly
    for item in &diagram.items {
        if let Item::Message { arrow, text, .. } = item {
            println!("Message: {} delay: {:?}", text, arrow.delay);
            assert_eq!(arrow.delay, Some(7), "Delay should be 7");
        }
    }
}

#[test]
fn generate_stress_test_svg() {
    let input = r#"title 🔥 Ultimate Stress Test: Microservices Chaos Engineering 🔥

participant "Mobile\nClient" as Mobile
participant "API Gateway\n(v2.3.1)" as Gateway
actor "System\nAdmin" as Admin
participant ":Auth::Service" as Auth
participant "User-DB\n[PostgreSQL]" as UserDB
participant "Redis::Cache\n(Cluster)" as Cache
participant "Kafka\nMessage\nBroker" as Kafka
participant "ML-Pipeline\n@tensorflow" as ML
participant "Notification\nHub" as Notify
participant "External\nPayment\nProvider" as Payment

Mobile->(1)Gateway: Quick request
Mobile->(3)Gateway: Medium delay
Mobile->(7)Gateway: Slow network
Gateway->(0)Auth: No delay (edge case)

Gateway->Auth: Solid filled
Auth-->Gateway: Dashed filled
Gateway->>Auth: Solid open
Auth-->>Gateway: Dashed open
Auth->Auth: Self message\nwith multiple\nlines of text\nfour lines total

note left of Mobile: 📱 Mobile client\nrunning iOS 17
note right of Gateway: 🚀 High availability\n99.99% uptime
note over Auth: 🔐 OAuth2 + JWT
note over UserDB, Cache: 💾 Data layer with\nread replicas and\ncache invalidation

note left of Admin
🔧 Admin capabilities:
- User management
- System monitoring
- Incident response
- Audit logging
- Config changes
end note

autonumber 100

alt 🟢 User authenticated
    Mobile->Gateway: GET /api/profile
    Gateway->+Auth: Validate token

    opt Token in cache
        Auth->Cache: GET token:{user_id}
        Cache-->Auth: Cached token data
    end

    alt Token valid
        Auth->UserDB: SELECT * FROM users

        loop Retry 3 times on failure
            UserDB-->Auth: User data

            opt Has preferences
                Auth->Cache: GET prefs:{user_id}

                alt Cache hit
                    Cache-->Auth: Preferences
                else Cache miss
                    Auth->UserDB: SELECT * FROM preferences
                    UserDB-->Auth: Preferences
                    Auth->Cache: SET prefs:{user_id}
                end
            end
        end

        Auth-->-Gateway: User profile + preferences

    else Token expired
        Auth-->Gateway: 401 Unauthorized
        Gateway->Auth: Refresh token

        alt Refresh successful
            Auth->UserDB: Update last_login
            Auth-->Gateway: New tokens
        else Refresh failed
            Auth-->Gateway: 403 Forbidden
            Gateway-->Mobile: Please re-login
        end
    end

else 🔴 User not authenticated
    Mobile->Gateway: Anonymous request
    Gateway-->Mobile: 401 Please login
end

autonumber off

state over Mobile: 📲 Authenticated
state over Gateway: ⚡ Processing
state over Auth: 🔒 Validating
state over UserDB: 💽 Querying
state over Cache: 🗄️ Caching

ref over Gateway, Auth, UserDB, Cache
<<input>> user_id, session_token
<<output>> user_profile, permissions
Complex Authentication Flow
See: auth-flow-v2.wsd
end ref

Gateway->Worker: Initialize job
Worker->ML: Process data
ML-->Worker: Results
Worker-->Gateway: Job complete
destroy Worker

Mobile->Gateway: POST /api/v2/users/profile/update\n{"name": "Test User", "email": "test@example.com", "preferences": {"theme": "dark", "notifications": true}}

Gateway->+Auth: Start auth
Auth->+UserDB: Query
UserDB->+Cache: Check
Cache-->-UserDB: Miss
UserDB-->-Auth: Data
Auth-->-Gateway: Done

Mobile->Gateway: Header: X-Custom-Header="value with spaces & special <chars>"
Gateway->Auth: Token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...

Mobile->Gateway: .
Gateway->Auth: ...
Auth->UserDB: →

ref over Kafka, ML, Notify, Payment
External Service Integration
1. Wait 1s
2. Retry with backoff
3. Alert on failure
end ref
"#;

    let diagram = parse(input).unwrap();
    let svg = render(&diagram);

    // Write to file for comparison
    let output_path = "/Users/kondomasaki/Documents/osd/opensequencediagrams-web/.claude/stress_test/OSD_Ultimate_Stress_Test_NEW.svg";
    fs::write(output_path, &svg).expect("Failed to write SVG");

    // Print first few lines to verify
    for line in svg.lines().take(5) {
        println!("{}", line);
    }

    // Verify SVG is generated
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Ultimate Stress Test"));
}
