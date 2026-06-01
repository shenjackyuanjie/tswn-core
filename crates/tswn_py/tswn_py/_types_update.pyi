"""战斗更新相关类型存根。"""

from __future__ import annotations

from ._types_replay import EventDto

class RunnerError(Exception):
    """Runner 初始化或运行时错误。"""

    ...

class RunUpdate:
    """单条战斗事件帧。"""
    @property
    def score(self) -> int:
        """视觉分值。"""
        ...
    @property
    def param(self) -> int | None:
        """可选数值参数。"""
        ...
    @property
    def delay0(self) -> int:
        """动画延迟 0。"""
        ...
    @property
    def delay1(self) -> int:
        """动画延迟 1。"""
        ...
    @property
    def message(self) -> str:
        """原始消息模板。"""
        ...
    @property
    def caster_id(self) -> int:
        """施法者 ID。"""
        ...
    @property
    def target_id(self) -> int:
        """目标 ID。"""
        ...
    @property
    def targets(self) -> list[int]:
        """多目标 ID 列表。"""
        ...
    def target_is_empty(self) -> bool:
        """返回目标列表是否为空。"""
        ...
    def get_update_type(self) -> str:
        """返回事件类型的调试字符串。"""
        ...
    def is_win(self) -> bool:
        """是否为胜利帧。"""
        ...
    def is_none(self) -> bool:
        """是否为空占位帧。"""
        ...
    def is_next_line(self) -> bool:
        """是否为换行帧。"""
        ...
    def msg(self) -> str:
        """返回占位符替换后的最终消息。"""
        ...

    def to_dict(self, rendered: bool = True) -> EventDto:
        """Return a structured event dictionary."""
        ...

class RunUpdates:
    """单回合内收集到的事件帧容器。"""
    def __init__(self) -> None:
        """创建一个空更新容器。"""
        ...
    @staticmethod
    def new_no_capture() -> RunUpdates:
        """创建不采集详细帧的容器。"""
        ...
    def clear(self) -> None:
        """清空当前容器内容。"""
        ...
    def reset(self) -> None:
        """重置容器状态。"""
        ...
    @property
    def id(self) -> int:
        """本批更新的唯一标识。"""
        ...
    @property
    def capture_updates(self) -> bool:
        """当前是否采集详细帧。"""
        ...
    @property
    def updates(self) -> list[RunUpdate]:
        """已收集的事件帧列表。"""
        ...
    @property
    def on_update_end(self) -> list[int]:
        """本批结束后触发回调的玩家 ID。"""
        ...
    def len(self) -> int:
        """返回帧数量。"""
        ...
    def is_empty(self) -> bool:
        """返回是否为空。"""
        ...
    def had_updates(self) -> bool:
        """返回本轮是否发生过有效更新。"""
        ...
