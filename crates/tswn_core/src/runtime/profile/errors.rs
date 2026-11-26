use super::*;

/// 默认 profile 组装失败：注册表构建错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultCustomRuntimeProfileError {
    Registry(ExtensionError),
}

impl From<ExtensionError> for DefaultCustomRuntimeProfileError {
    /// 把注册表构建错误转换为 [`DefaultCustomRuntimeProfileError::Registry`] 变体。
    fn from(error: ExtensionError) -> Self { Self::Registry(error) }
}

/// 召唤物模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2SummonTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2SummonTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2SummonTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2SummonTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2SummonTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2SummonTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2SummonTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 幻影模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ShadowTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ShadowTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2ShadowTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ShadowTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2ShadowTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ShadowTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2ShadowTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 亡灵模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ZombieTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ZombieTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2ZombieTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ZombieTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2ZombieTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ZombieTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2ZombieTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 使魔模板 overlay 组合导入失败：任一步骤错误或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2MinionOverlayImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Summon(CustomBed2SummonTemplateImportError),
    Shadow(CustomBed2ShadowTemplateImportError),
    Zombie(CustomBed2ZombieTemplateImportError),
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2MinionOverlayImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2MinionOverlayImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2MinionOverlayImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2MinionOverlayImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2SummonTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把召唤模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Summon`] 变体。
    fn from(error: CustomBed2SummonTemplateImportError) -> Self { Self::Summon(error) }
}

impl From<CustomBed2ShadowTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把影袭模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Shadow`] 变体。
    fn from(error: CustomBed2ShadowTemplateImportError) -> Self { Self::Shadow(error) }
}

impl From<CustomBed2ZombieTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把亡灵模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Zombie`] 变体。
    fn from(error: CustomBed2ZombieTemplateImportError) -> Self { Self::Zombie(error) }
}

impl From<SlotError> for CustomBed2MinionOverlayImportError {
    /// 把槽错误转换为 [`CustomBed2MinionOverlayImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 自定义 Runtime 导入的顶层错误，汇总 roster、overlay、初始化与就绪校验失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomRuntimeImportError {
    Bed2Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Bed2MinionOverlay(CustomBed2MinionOverlayImportError),
    BattleInit(RuntimeBattleInitError),
    NotReady(RuntimeReadyError),
}

impl From<CustomBed2RosterImportError> for CustomRuntimeImportError {
    /// 把 bed2 roster 错误转换为 [`CustomRuntimeImportError::Bed2Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Bed2Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomRuntimeImportError {
    /// 把混合 roster 错误转换为 [`CustomRuntimeImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2MinionOverlayImportError> for CustomRuntimeImportError {
    /// 把使魔 overlay 导入错误转换为 [`CustomRuntimeImportError::Bed2MinionOverlay`] 变体。
    fn from(error: CustomBed2MinionOverlayImportError) -> Self { Self::Bed2MinionOverlay(error) }
}

impl From<RuntimeBattleInitError> for CustomRuntimeImportError {
    /// 把战斗初始化错误转换为 [`CustomRuntimeImportError::BattleInit`] 变体。
    fn from(error: RuntimeBattleInitError) -> Self { Self::BattleInit(error) }
}

impl From<RuntimeReadyError> for CustomRuntimeImportError {
    /// 把就绪校验错误转换为 [`CustomRuntimeImportError::NotReady`] 变体。
    fn from(error: RuntimeReadyError) -> Self { Self::NotReady(error) }
}

/// 混合 roster 导入失败：给出队伍、行内位置、原文与原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMixedRosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
    pub message: String,
}
