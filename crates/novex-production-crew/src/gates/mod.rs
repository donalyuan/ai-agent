//! 质量闸门模块：在角色执行关键节点检查产物质量、预算和权限

pub mod budget_gate;
pub mod gate_trait;
pub mod producer_gate;
pub mod publish_gate;
pub mod quality_gate;
pub mod resource_safety_gate;
pub mod script_approval_gate;
pub mod technical_feasibility_gate;

pub use gate_trait::{Gate, GateContext, GateDecision};

use std::collections::HashMap;
use std::sync::Arc;

/// 闸门注册表：按名称索引所有已注册的 Gate 实现
pub struct GateRegistry {
    gates: HashMap<String, Arc<dyn Gate>>,
}

impl GateRegistry {
    pub fn new() -> Self {
        Self {
            gates: HashMap::new(),
        }
    }

    /// 注册一个 Gate 实现
    pub fn register(&mut self, gate: Arc<dyn Gate>) {
        self.gates.insert(gate.name().to_string(), gate);
    }

    /// 按名称查找 Gate
    pub fn get(&self, name: &str) -> Option<Arc<dyn Gate>> {
        self.gates.get(name).cloned()
    }

    /// Bootstrap：注册全部内置 Gate
    pub fn bootstrap() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(producer_gate::ProducerGate));
        registry.register(Arc::new(script_approval_gate::ScriptApprovalGate));
        registry.register(Arc::new(
            technical_feasibility_gate::TechnicalFeasibilityGate,
        ));
        registry.register(Arc::new(quality_gate::QualityGate));
        registry.register(Arc::new(resource_safety_gate::ResourceSafetyGate));
        // BudgetGate 仅保留给非目标 Fast Lane；Full Crew 固定计划禁止引用它。
        registry.register(Arc::new(budget_gate::BudgetGate));
        registry.register(Arc::new(publish_gate::PublishGate));
        registry
    }
}

impl Default for GateRegistry {
    fn default() -> Self {
        Self::bootstrap()
    }
}
