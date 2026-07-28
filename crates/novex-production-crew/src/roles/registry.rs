//! 角色注册表：运行时内存索引，提供角色查找和执行序列验证

use crate::error::ProductionError;
use crate::roles::definition::RoleDefinition;
use crate::roles::loader::RoleLoader;
use crate::ProductionResult;
use std::collections::HashMap;
use std::path::Path;

pub struct RoleRegistry {
    /// 以 role_key 为键的角色索引，O(1) 查找
    roles: HashMap<String, RoleDefinition>,
}

impl RoleRegistry {
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
        }
    }

    /// 从磁盘上的 roles 目录加载全部角色定义，注册为运行时索引。
    ///
    /// `roles_dir` 通常为 `{crate_root}/roles/`（容器内对应 `/app/crates/novex-production-crew/roles/`）。
    pub fn bootstrap(roles_dir: &Path) -> ProductionResult<Self> {
        let defs = RoleLoader::load_from_dir(roles_dir)?;
        let mut registry = Self::new();
        for def in defs {
            registry.register(def);
        }
        Ok(registry)
    }

    /// 注册角色，同 role_key 会覆盖
    pub fn register(&mut self, def: RoleDefinition) {
        self.roles.insert(def.role_key.clone(), def);
    }

    /// 按 role_key 查找角色，不存在时返回 RoleNotFound 错误
    pub fn get(&self, role_key: &str) -> ProductionResult<&RoleDefinition> {
        self.roles
            .get(role_key)
            .ok_or_else(|| ProductionError::RoleNotFound {
                role_key: role_key.to_string(),
            })
    }

    pub fn list_all(&self) -> Vec<&RoleDefinition> {
        self.roles.values().collect()
    }

    /// 验证角色执行序列合法性（输入产物依赖检查）
    ///
    /// 遍历序列中每个角色，确保其所需输入产物至少有一个前置角色会产出。
    /// 若某输入产物无任何前置角色产出，记录 debug 日志（视为来自用户外部输入，不阻断流程）。
    pub fn validate_sequence(&self, sequence: &[String]) -> ProductionResult<()> {
        // 累计前置角色已产出的产物集合
        let mut available_outputs: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for role_key in sequence {
            let def = self.get(role_key)?;
            for required in &def.input_artifacts {
                let key = format!("{:?}", required);
                if !available_outputs.contains(&key) {
                    // 非阻塞：部分输入由用户直接提供（如初始 creative_brief）
                    tracing::debug!(
                        role = %role_key,
                        artifact = %key,
                        "输入产物无前置角色产出，预期来自用户外部输入"
                    );
                }
            }
            for output in &def.output_artifacts {
                available_outputs.insert(format!("{:?}", output));
            }
        }
        Ok(())
    }
}

impl Default for RoleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::definition::{Lifecycle, PromptRef, RoleDefinition};

    fn make_role(key: &str) -> RoleDefinition {
        RoleDefinition {
            role_key: key.to_string(),
            role_name: key.to_string(),
            responsibilities: vec![],
            input_artifacts: vec![],
            output_artifacts: vec![],
            allowed_tools: vec![],
            prompt_definition_ref: PromptRef {
                key: format!("{}.general", key),
                version: "@1".to_string(),
            },
            lifecycle: Lifecycle::Active,
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = RoleRegistry::new();
        registry.register(make_role("producer"));
        assert!(registry.get("producer").is_ok());
        assert!(registry.get("nonexistent").is_err());
    }

    #[test]
    fn test_list_all() {
        let mut registry = RoleRegistry::new();
        registry.register(make_role("producer"));
        registry.register(make_role("screenwriter"));
        assert_eq!(registry.list_all().len(), 2);
    }

    #[test]
    fn test_validate_sequence_unknown_role_returns_err() {
        let registry = RoleRegistry::new();
        let seq = vec!["nonexistent".to_string()];
        assert!(registry.validate_sequence(&seq).is_err());
    }

    #[test]
    fn test_validate_sequence_valid() {
        let mut registry = RoleRegistry::new();
        registry.register(make_role("producer"));
        registry.register(make_role("screenwriter"));
        let seq = vec!["producer".to_string(), "screenwriter".to_string()];
        assert!(registry.validate_sequence(&seq).is_ok());
    }
}
