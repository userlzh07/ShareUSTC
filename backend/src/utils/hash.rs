use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;

/// 使用 Argon2id 算法对密码进行哈希
///
/// # Arguments
/// * `password` - 明文密码
///
/// # Returns
/// * `Ok(String)` - 哈希后的密码
/// * `Err(String)` - 错误信息
pub fn hash_password(password: &str) -> Result<String, String> {
    // 使用 Argon2id 算法
    let argon2 = Argon2::default();

    // 生成随机盐
    let salt = SaltString::generate(&mut OsRng);

    // 哈希密码
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("密码哈希失败: {}", e))?
        .to_string();

    Ok(password_hash)
}

/// 验证密码
///
/// # Arguments
/// * `password` - 明文密码
/// * `hash` - 存储的密码哈希
///
/// # Returns
/// * `Ok(bool)` - 验证结果
/// * `Err(String)` - 错误信息
pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    // 解析存储的密码哈希
    let parsed_hash = PasswordHash::new(hash).map_err(|e| format!("密码哈希格式错误: {}", e))?;

    // 验证密码
    let argon2 = Argon2::default();
    let result = argon2.verify_password(password.as_bytes(), &parsed_hash);

    match result {
        Ok(_) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(format!("密码验证失败: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "test123456";
        let hash = hash_password(password).expect("哈希密码失败");

        // 确保哈希值不为空且与明文不同
        assert!(!hash.is_empty());
        assert_ne!(hash, password);

        // 确保哈希值包含 Argon2 标识
        assert!(hash.contains("argon2id"));
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "test123456";
        let hash = hash_password(password).expect("哈希密码失败");

        let result = verify_password(password, &hash).expect("验证密码失败");
        assert!(result);
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "test123456";
        let wrong_password = "wrongpassword";
        let hash = hash_password(password).expect("哈希密码失败");

        let result = verify_password(wrong_password, &hash).expect("验证密码失败");
        assert!(!result);
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let password = "test123456";
        let hash1 = hash_password(password).expect("哈希密码失败");
        let hash2 = hash_password(password).expect("哈希密码失败");

        // 相同密码应该产生不同的哈希（因为使用了随机盐）
        assert_ne!(hash1, hash2);

        // 但两者都应该能验证成功
        assert!(verify_password(password, &hash1).expect("验证失败"));
        assert!(verify_password(password, &hash2).expect("验证失败"));
    }
}
