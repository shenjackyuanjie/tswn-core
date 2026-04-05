"""Runner 与 PreparedRunner 类型存根。"""

from __future__ import annotations

from ._types_engine import Storage, WorldState
from ._types_rc4 import RC4
from ._types_update import RunUpdates

class PreparedRunner:
    """预处理后的分组输入，可重复用于构造 Runner。"""

    ...

class Runner:
    """战斗引擎的主要 Python 入口。"""
    @staticmethod
    def new_from_namerena_raw(raw_str: str) -> Runner:
        """从名竞原始输入构造 Runner。"""
        ...
    @staticmethod
    def split_namerena_into_groups(raw_str: str) -> tuple[list[list[str]], list[str]]:
        """将原始输入拆成 `(groups, seed)`。"""
        ...
    @staticmethod
    def new_from_groups_with_seed(groups: list[list[str]], seed: list[str]) -> Runner:
        """从分组输入和 seed 构造 Runner。"""
        ...
    @staticmethod
    def new_from_groups_with_seed_and_eval_rq(
        groups: list[list[str]], seed: list[str], eval_rq: float
    ) -> Runner:
        """从分组输入、seed 和显式 eval_rq 构造 Runner。"""
        ...
    @staticmethod
    def prepare_groups(groups: list[list[str]]) -> PreparedRunner:
        """预处理分组输入以便复用。"""
        ...
    @staticmethod
    def prepare_groups_with_eval_rq(
        groups: list[list[str]], eval_rq: float
    ) -> PreparedRunner:
        """使用显式 eval_rq 预处理分组输入。"""
        ...
    @staticmethod
    def new_from_prepared_with_seed(
        prepared: PreparedRunner, seed: list[str]
    ) -> Runner:
        """从预处理结果和 seed 构造 Runner。"""
        ...
    def main_round(self) -> RunUpdates:
        """推进到下一个主回合并返回更新。"""
        ...
    def round_tick(self, update: RunUpdates) -> None:
        """执行一个 tick，并把结果追加到给定容器。"""
        ...
    def round_tick_new_update(self) -> RunUpdates:
        """执行一个 tick 并返回新建的更新容器。"""
        ...
    def round_tick_new_update_no_capture(self) -> RunUpdates:
        """执行一个不采集详细帧的 tick。"""
        ...
    def run_to_completion(self) -> bool:
        """一直运行到结束，返回是否分出胜者。"""
        ...
    @property
    def storage(self) -> Storage:
        """底层存储接口。"""
        ...
    @property
    def world_state(self) -> WorldState:
        """当前世界状态。"""
        ...
    @property
    def input_groups(self) -> list[list[int]]:
        """原始输入顺序对应的队伍 roster。"""
        ...
    @property
    def rc4(self) -> RC4:
        """底层 RC4 状态。"""
        ...
    def have_winner(self) -> bool:
        """当前是否已有赢家。"""
        ...
    def alives_flat(self) -> list[int]:
        """所有存活玩家 ID 的扁平列表。"""
        ...
    def alives(self) -> list[list[int]]:
        """按组返回存活玩家 ID。"""
        ...
    def all_plrs(self) -> list[int]:
        """全部玩家 ID，含死亡。"""
        ...
    def all_plr_len(self) -> int:
        """全部玩家数量，含死亡。"""
        ...
