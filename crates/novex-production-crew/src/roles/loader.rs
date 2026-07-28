//! 角色定义加载器：从 YAML manifest 文件读取并验证角色定义

use crate::error::ProductionError;
use crate::roles::definition::RoleDefinition;
use crate::ProductionResult;
use std::path::Path;

pub struct RoleLoader;

impl RoleLoader {
    /// 从 YAML 文件加载单个角色定义
    pub fn load_from_file(path: &Path) -> ProductionResult<RoleDefinition> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ProductionError::YamlParse(format!("读取角色文件失败 {:?}: {}", path, e))
        })?;
        let def: RoleDefinition = serde_yaml::from_str(&content).map_err(|e| {
            ProductionError::YamlParse(format!("解析角色 YAML 失败 {:?}: {}", path, e))
        })?;
        Self::validate(&def)?;
        Ok(def)
    }

    /// 从目录批量加载所有 `*.yaml` 角色定义，跳过 `registry.yaml`
    pub fn load_from_dir(dir: &Path) -> ProductionResult<Vec<RoleDefinition>> {
        let mut roles = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| {
            ProductionError::YamlParse(format!("读取角色目录失败 {:?}: {}", dir, e))
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| ProductionError::YamlParse(e.to_string()))?;
            let path = entry.path();
            let is_yaml = path.extension().and_then(|e| e.to_str()) == Some("yaml");
            let is_registry = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == "registry.yaml")
                .unwrap_or(false);
            if is_yaml && !is_registry {
                roles.push(Self::load_from_file(&path)?);
            }
        }
        Ok(roles)
    }

    /// 验证角色定义完整性
    /// 确保 role_key 和 prompt_definition_ref.key 非空
    pub fn validate(def: &RoleDefinition) -> ProductionResult<()> {
        if def.role_key.is_empty() {
            return Err(ProductionError::YamlParse("角色 role_key 不能为空".into()));
        }
        if def.prompt_definition_ref.key.is_empty() {
            return Err(ProductionError::YamlParse(format!(
                "角色 {} 的 prompt_definition_ref.key 不能为空",
                def.role_key
            )));
        }
        Ok(())
    }
}
