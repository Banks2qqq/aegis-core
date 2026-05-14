fn dangerous_function(user_input: &str) {
    // Опасный вызов
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(user_input);
    let _ = cmd.output();
    
    // Unsafe блок
    unsafe {
        std::ptr::write_volatile(0xdeadbeef as *mut u8, 42);
    }
    
    // Ещё один опасный вызов
    let _ = std::process::Command::new("rm")
        .arg("-rf")
        .arg("/tmp/test");
}