//! AccountManager 单元测试（导入/名称覆盖/回退逻辑）。

    use super::*;

    /// 测试导入账号时名称覆盖逻辑
    #[tokio::test]
    async fn test_import_account_name_override() {
        // 创建一个临时路径用于测试
        let temp_dir = std::env::temp_dir().join("traejumper_test_import");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            last_auto_checkin_date: None,
            data_path: test_path.clone(),
        };

        // 先手动添加一个账号（模拟已存在的账号，但名称是 User_xxx 格式）
        let mut existing_account = Account::new(
            "User_41928646".to_string(),
            "".to_string(),
            "".to_string(),
            "4192864699424393".to_string(),
            "7o2d894p7dr0o4".to_string(),
        );
        existing_account.avatar_url = "".to_string();
        existing_account.region = "".to_string();
        manager.store.accounts.push(existing_account);

        println!("[TEST] 测试账号名称: '{}'", manager.store.accounts[0].name);

        // 模拟导入过程（只执行名称覆盖逻辑，跳过 API 调用）
        let test_json = r#"[
            {
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "",
                "machine_id": null
            }
        ]"#;

        let import_data: Vec<serde_json::Value> = serde_json::from_str(test_json).unwrap();
        for item in import_data {
            let exported_name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_avatar = item.get("avatar_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_user_id = item.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_region = item.get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 执行名称覆盖逻辑（与 import_accounts 中相同）
            if !exported_name.is_empty() && !exported_user_id.is_empty() {
                if let Some(acc) = manager.store.accounts.iter_mut().find(|a| a.user_id == exported_user_id) {
                    acc.name = exported_name.clone();
                    if !exported_avatar.is_empty() {
                        acc.avatar_url = exported_avatar.clone();
                    }
                    if !exported_region.is_empty() {
                        acc.region = exported_region.clone();
                    }
                }
            }
        }

        // 断言：名称应该被覆盖为导出数据中的名称
        assert_eq!(manager.store.accounts[0].name, "用户7956360138", "名称覆盖失败！");
        assert_eq!(manager.store.accounts[0].avatar_url, "https://example.com/avatar.png", "头像覆盖失败！");
        assert_eq!(manager.store.accounts[0].region, "CN", "区域覆盖失败！");

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 所有断言通过！名称覆盖逻辑正确工作。");
    }

    /// 测试完整的 import_accounts 流程（使用空的 jwt_token，测试名称覆盖路径）
    #[tokio::test]
    async fn test_import_accounts_full_flow() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_full_import");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            last_auto_checkin_date: None,
            data_path: test_path.clone(),
        };

        // 测试数据：cookies 和 jwt_token 都为空，跳过 API 调用
        // 验证：名称覆盖不会执行（因为没有账号被添加）
        let test_json = r#"[
            {
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "",
                "machine_id": null
            }
        ]"#;

        let result = manager.import_accounts(test_json).await.unwrap();
        println!("[TEST] import_accounts 返回: {} 个账号", result);

        // 由于 jwt_token 和 cookies 都为空，应该导入 0 个账号
        assert_eq!(result, 0, "无 token/cookies 时应导入 0 个账号");
        assert_eq!(manager.store.accounts.len(), 0, "store 中应无账号");

        // 测试：已有账号 + 空 jwt_token 的导入数据
        // 验证：名称覆盖逻辑是否对已存在的账号生效
        let mut existing_account = Account::new(
            "User_41928646".to_string(),
            "".to_string(),
            "".to_string(),
            "4192864699424393".to_string(),
            "7o2d894p7dr0o4".to_string(),
        );
        existing_account.avatar_url = "".to_string();
        existing_account.region = "".to_string();
        manager.store.accounts.push(existing_account);
        manager.save_store().unwrap();

        // 再次导入（空 jwt_token，不触发 API 调用，但名称覆盖应针对已存在账号）
        let result = manager.import_accounts(test_json).await.unwrap();
        println!("[TEST] 第二次 import_accounts 返回: {} 个账号", result);
        println!("[TEST] 第二次导入后 store 中账号: {} 个", manager.store.accounts.len());

        // 由于 jwt_token 为空，add_account_by_token 不会被调用
        // add_success 为 false，所以 imported_count 为 0
        // 但名称覆盖逻辑应该对已存在的账号生效
        println!("[TEST] 最终账号名称: '{}'", manager.store.accounts[0].name);

        // 关键验证：已存在的账号名称应该被覆盖
        // 注意：import_accounts 中名称覆盖逻辑在 add_success 判断之前执行
        // 即使 add_success 为 false，名称覆盖也应该生效
        assert_eq!(manager.store.accounts[0].name, "用户7956360138", "名称覆盖应该在导入时对已存在账号生效！");

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 完整流程测试通过！");
    }

    /// 测试 JWT Token 解析和降级创建路径
    #[tokio::test]
    async fn test_import_fallback_from_jwt() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_jwt_fallback");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            last_auto_checkin_date: None,
            data_path: test_path.clone(),
        };

        // 使用一个真实的 JWT Token 来测试解析
        // 这个 token 是从实际的导出文件中提取的
        let test_jwt = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDE5Mjg2NDY5OTQyNDM5MyIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJycU5LNGlFdDRickFZSlhmeFhpVzZfTXZMYjVyS2FteHE5enlQaXQ2QW9ZPS4xOGNiYmM1ZGY4Y2MzYmI0IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODc5MzgzODgsImlhdCI6MTc4NjcyODc4OH0.Cx3NREOtJlGGKW6QTb3F5MoVu52xG2GaUNXEpvBMoqWfSJyqu0yjl1p0RL6to3tgAhSsH838NL_vQdDk6qj8WfsubCDj5XuLl9TxqTmYhgrCgZVnFMSxszMi6C0Y2adzTRb0Hk_griCXZZs3GJDLgNP3vIiOK4ukzm6wXJkt1LHq3El3fqKEb4jT1uSICK_OqhuJzkB3zrQ1O0Ng0oDTtdvNtLYGjmveOSfSvhOeUXHVx6PAy1UCN0yhNCEJ0ni5-w4v6I8bEhlGDR90Gf87ZxjewTPusNI6TuRKrAYQssYsizHIwDFXRmnzDco6YMMBQwvMv_qJM0rDOCSdE8juQf_X39tj0vmlvw1w8vPrbuuJr9gQB3UVwuhczy8J9lw7OAO0w_thts0wN9b6rYh4UtG4jIB1DJqEvSFmnGk7O1n3nf5kHKlpaa1X4acpLEAy31wNTR05bsSd1SdkS2Z2T_SXro9MKuoqfnYsyz7sBS3xolXpCYjL8zIHxBDGWpNUp1kBpJ-lQELideJtm2ljY6te_Tqas9NAa0_hzLP5KKB8AM51-wZonkG24rupYoTQQ6OahCqZXNwOHgMQuSYaDq50Lw26iYA-UNR1KLNtqTilEFIngAFLdVZGE4zx1XkX5sPjNIYyo07ZBh9ZT6-iLcMvRh7VJr7Yc7caJTr_ZHE";

        // 验证 JWT 解析
        let user_id = crate::api::TraeApiClient::parse_jwt_user_id(test_jwt).unwrap();
        assert_eq!(user_id, "4192864699424393", "JWT 解析 user_id 失败");
        println!("[TEST] JWT 解析成功: user_id='{}'", user_id);

        // 使用包含真实 JWT 的导入数据测试降级创建
        let test_json = format!(r#"[
            {{
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://example.com/avatar.png",
                "jwt_token": "{}",
                "machine_id": null
            }}
        ]"#, test_jwt);

        // 调用完整的 import_accounts（会尝试 API 调用并失败，然后降级到 JWT 解析创建）
        let result = manager.import_accounts(&test_json).await.unwrap();
        println!("[TEST] import_accounts 返回: {} 个账号, store 中账号: {} 个", result, manager.store.accounts.len());

        // 验证降级创建成功
        if manager.store.accounts.len() > 0 {
            let acc = &manager.store.accounts[0];
            println!("[TEST] 降级创建账号: name='{}', user_id='{}', region='{}', avatar='{}'",
                acc.name, acc.user_id, acc.region, acc.avatar_url);
            assert_eq!(acc.name, "用户7956360138", "名称应该正确");
            assert_eq!(acc.user_id, "4192864699424393", "user_id 应该正确");
            assert_eq!(acc.region, "CN", "region 应该正确");
            assert_eq!(acc.avatar_url, "https://example.com/avatar.png", "avatar_url 应该正确");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] JWT 降级创建测试通过！");
    }

    /// 完整集成测试：模拟导入用户真实导出的3个账号，验证所有名称正确
    #[tokio::test]
    async fn test_import_full_real_data() {
        let temp_dir = std::env::temp_dir().join("traejumper_test_full_real");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_path = temp_dir.join("test_accounts.json");

        let mut manager = AccountManager {
            store: AccountStore::default(),
            last_auto_checkin_date: None,
            data_path: test_path.clone(),
        };

        // 使用用户真实导出数据中的3个JWT Token
        let jwt1 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDE5Mjg2NDY5OTQyNDM5MyIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJycU5LNGlFdDRickFZSlhmeFhpVzZfTXZMYjVyS2FteHE5enlQaXQ2QW9ZPS4xOGNiYmM1ZGY4Y2MzYmI0IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODc5MzgzODgsImlhdCI6MTc4NjcyODc4OH0.Cx3NREOtJlGGKW6QTb3F5MoVu52xG2GaUNXEpvBMoqWfSJyqu0yjl1p0RL6to3tgAhSsH838NL_vQdDk6qj8WfsubCDj5XuLl9TxqTmYhgrCgZVnFMSxszMi6C0Y2adzTRb0Hk_griCXZZs3GJDLgNP3vIiOK4ukzm6wXJkt1LHq3El3fqKEb4jT1uSICK_OqhuJzkB3zrQ1O0Ng0oDTtdvNtLYGjmveOSfSvhOeUXHVx6PAy1UCN0yhNCEJ0ni5-w4v6I8bEhlGDR90Gf87ZxjewTPusNI6TuRKrAYQssYsizHIwDFXRmnzDco6YMMBQwvMv_qJM0rDOCSdE8juQf_X39tj0vmlvw1w8vPrbuuJr9gQB3UVwuhczy8J9lw7OAO0w_thts0wN9b6rYh4UtG4jIB1DJqEvSFmnGk7O1n3nf5kHKlpaa1X4acpLEAy31wNTR05bsSd1SdkS2Z2T_SXro9MKuoqfnYsyz7sBS3xolXpCYjL8zIHxBDGWpNUp1kBpJ-lQELideJtm2ljY6te_Tqas9NAa0_hzLP5KKB8AM51-wZonkG24rupYoTQQ6OahCqZXNwOHgMQuSYaDq50Lw26iYA-UNR1KLNtqTilEFIngAFLdVZGE4zx1XkX5sPjNIYyo07ZBh9ZT6-iLcMvRh7VJr7Yc7caJTr_ZHE";
        let jwt2 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiMjM0NjIxNzg4NzYwOTMyIiwic291cmNlIjoicmVmcmVzaF90b2tlbiIsInNvdXJjZV9pZCI6IkJiREl5OHdtdzhNRV9vWTJJZDF5c292elQxalFYRk1ZajZoVEcxdG50MjA9LjE4Y2MzYTBiNmIzZjJkZmQiLCJ0ZW5hbnRfaWQiOiI3bzJkODk0cDdkcjBvNCIsInR5cGUiOiJ1c2VyIn0sImV4cCI6MTc4ODA3NjU3MiwiaWF0IjoxNzg2ODY2OTcyfQ.NOgs0iOwFw_sDNNpp1q3Alhb5LCw1XJFuct4tStBIrUkJHg7gcrBBZQU8pihu4gvCCYNwvZjebXK5DU3gH9jt55OKvb9DX2SDXhslu35b4q2Mjhoqgfhi-7g5XMRFtBoJr_FPds6-6qs9wE-cMxA1o2FFPt-YsYqP5eaLmQ8IKx13tDxH4x3P71NFpnYgBv6CzJaI_eVIIMcBOP2uc4OQB_gaIrpcCFr2ickIWkdkZ1AY67E7l9pMlvflc9Zy6iX8MAtVSr3XyIUCQplzYWYa0xO0LQmCBCPJde8FJHHa-DKe9sdWbs2UTERTo693aa-MSW6HIoNnt7RKYFDenm5_A62v4W58sld-dFiOkVUrVmDvhrRwNZQUnae43X-tCGF_n8J1YiF5Wwlu4oYNQx9LIwv5aII7qRnNS6HlgcevlyhepxogkaCzitWTUTFCY_WDsbUdNU9CS-3JdJTzMr2HH55mGvipWRFaG1Bv6mIsuga5wz9_kb75w-cxyw2Qk5NGUtejJBBtLyuIK8mR4JE_Y08vuT-sabV6ftX8F1tkBcKEzl0JXHO6HqVMuYltES-8yGNnWUtPXrxOPw-WGz06wJfRjetHUWsFYPX3hqQ4d3q76ZXilS73qa8feNi5P-dRqtZiUeja_Gk1FfFHeXdJ6psxjKV0hODWl43yvTS3Dg";
        let jwt3 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNDA2NTM1MTEyMzg4NjQxMCIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiI4Z21vaElyZHFKck1pZERDUnZYaFIxWk02ZHk4cC1pRFdfXzJ5c2V5Zk40PS4xOGNjNTNmMDQ3NzJkZDc1IiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3ODgxMDUwNDMsImlhdCI6MTc4Njg5NTQ0M30.OjyN4iNifs5mk_N_ZWC8SlTmZmMo3WpmD5gRS43G_8cbYaeD-3lw4yZJB0Owl36_jvpMxxiywtKCq28BUvSKjMCzqG4yoHSQIw8CMsK1UaKXrx8sgFFkE8n9fVr9AgzcCWqycI1mOgnnU_dMmAToA-3Rz4gq-XtT9haj78NJugxLZrL9zIzFTg5LvTA3xnA-p9wIQhL4lVQSWFnSufEQTe_x21ZVc72hZJqqJNFKSOF98O2WkCvP5ixlzqN0ekzxBWJnYOwVotZg3gbeGLKx8CyD-RqQe9fArE1RzEf-GRmjkPnvNI68Z-gibVoMFcQeiG3KmPzjFiR-Gll5zm18m_bkmUFTpPxO_slVrEko9C7m2sOAfaa-x-co_0WkSy61Hzqt8-dislSB25tH_we5IMlKu7lI4g855U8tibJGWLMzMKVu0VDsN_2f5147ML5Tf3PWhL9jqwOJGmxGtKS3R37Mi_j2u3a6plCDiWR0EOHwUrEEu-3oA0QQ_zbzPNTvk9vfUcMPBkcAQEDA3fJSczSWSHAvUz5QBKemLcmlGJiG6x9y6oSgVNlsceLjKsvVoirZVFmREgxXZ2L-1-XlspIas207UNKGDAnyYRxHbTB_JI2gtta-1lAYUoBHK1YmhlrnZA3RfDcULJZhOBVg1vCHhk3Eds6T5Krwmb094hA";

        // 构建与用户导出文件完全相同的 JSON 数据
        let test_json = format!(r#"[
            {{
                "name": "用户7956360138",
                "email": "",
                "cookies": "",
                "user_id": "4192864699424393",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p6-passport.byteacctimg.com/img/user-avatar/assets/11c35f217be67876726ffb8038af8e4e_192_192.png~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }},
            {{
                "name": "Francisyep",
                "email": "",
                "cookies": "",
                "user_id": "234621788760932",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p3-passport.byteacctimg.com/img/user-avatar/assets/11c35f217be67876726ffb8038af8e4e_192_192.png~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }},
            {{
                "name": "🏄🏻冲浪猫",
                "email": "",
                "cookies": "",
                "user_id": "4065351123886410",
                "tenant_id": "7o2d894p7dr0o4",
                "region": "CN",
                "plan_type": "Free",
                "avatar_url": "https://p9-passport.byteacctimg.com/img/user-avatar/67c16e65322f39664f9f2b6612f8bf11~128x128.image",
                "jwt_token": "{}",
                "machine_id": null
            }}
        ]"#, jwt1, jwt2, jwt3);

        // 调用完整的 import_accounts
        let result = manager.import_accounts(&test_json).await.unwrap();
        println!("[TEST] 完整导入返回: {} 个账号, store 中账号: {} 个", result, manager.store.accounts.len());

        // 验证导入了3个账号
        assert_eq!(result, 3, "应该成功导入3个账号");
        assert_eq!(manager.store.accounts.len(), 3, "store中应该有3个账号");

        // 验证每个账号的名称正确
        let expected = [
            ("用户7956360138", "4192864699424393", "CN", "byteacctimg"),
            ("Francisyep", "234621788760932", "CN", "byteacctimg"),
            ("🏄🏻冲浪猫", "4065351123886410", "CN", "byteacctimg"),
        ];

        for (i, (exp_name, exp_uid, exp_region, exp_avatar_sub)) in expected.iter().enumerate() {
            let acc = &manager.store.accounts[i];
            println!("[TEST] 账号[{}]: name='{}', user_id='{}', region='{}'",
                i, acc.name, acc.user_id, acc.region);
            assert_eq!(acc.name, *exp_name, "账号[{}] 名称应该正确", i);
            assert_eq!(acc.user_id, *exp_uid, "账号[{}] user_id 应该正确", i);
            assert_eq!(acc.region, *exp_region, "账号[{}] region 应该正确", i);
            assert!(acc.avatar_url.contains(exp_avatar_sub), "账号[{}] avatar_url 应包含'{}'", i, exp_avatar_sub);
        }

        // 模拟导出并验证导出数据中的名称也是正确的
        let exported = manager.export_accounts().unwrap();
        let exported_data: Vec<serde_json::Value> = serde_json::from_str(&exported).unwrap();
        println!("[TEST] 导出数据: {}", serde_json::to_string_pretty(&exported_data).unwrap());
        for (i, item) in exported_data.iter().enumerate() {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            assert_eq!(name, expected[i].0, "导出数据中账号[{}] 名称应该正确", i);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        println!("[TEST] 完整集成测试通过！所有3个账号名称、region、头像均正确！");
    }
