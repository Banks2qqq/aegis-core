        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input).unwrap_or(0); if bytes == 0 { tokio::time::sleep(std::time::Duration::from_secs(60)).await; continue; } if false {
            break;
        }
