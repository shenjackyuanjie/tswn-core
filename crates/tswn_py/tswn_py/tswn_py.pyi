"""
tswn_py 扩展模块类型存根（PyO3 生成的 C 扩展）。

此文件描述 tswn_py.tswn_py 扩展模块导出的所有公开 API。
通过 tswn_py/__init__.py 的 `from .tswn_py import *` 后，
这些符号也可直接从 `tswn_py` 顶层访问。
"""

from __future__ import annotations

from typing import final

# ---------------------------------------------------------------------------
# 模块级函数
# ---------------------------------------------------------------------------

def wrapper_version_str() -> str:
    """返回 tswn_py 绑定层（Rust crate）的版本字符串。"""
    ...

def core_version_str() -> str:
    """返回 tswn_core 引擎的版本字符串。"""
    ...

def name_to_png_base64(name: str) -> str:
    """
    将玩家名字渲染为 PNG 并返回 Base64 编码字符串。

    Parameters
    ----------
    name:
        玩家名字字符串。
    """
    ...

def name_to_png_bytes(name: str) -> bytes:
    """
    将玩家名字渲染为 PNG 并返回原始字节数据。

    Parameters
    ----------
    name:
        玩家名字字符串。
    """
    ...

# ---------------------------------------------------------------------------
# 异常
# ---------------------------------------------------------------------------

class RunnerError(Exception):
    """
    Runner 初始化或运行时错误。

    由 ``Runner.new_from_namerena_raw`` 在输入解析失败时抛出。
    继承自 ``Exception``，可直接 ``except RunnerError`` 捕获。
    """
    ...

# ---------------------------------------------------------------------------
# RunUpdate — 单条战斗事件帧
# ---------------------------------------------------------------------------

@final
class RunUpdate:
    """
    单条战斗事件消息帧，由引擎在每个 tick 内部生成，不可从 Python 直接实例化。

    消息模板规则
    ------------
    ``message`` 字段可包含三种占位符：

    * ``[0]`` — 施法者 ID
    * ``[1]`` — 目标 ID
    * ``[2]`` — ``param``（若有）或 ``targets`` 列表（逗号拼接）

    调用 :meth:`msg` 可获得替换完成后的最终字符串。
    """

    @property
    def score(self) -> int:
        """视觉分值（u32），用于 UI 动画强度控制。"""
        ...

    @property
    def param(self) -> int | None:
        """
        可选数值参数（u32），对应消息中 ``[2]`` 的纯数字情形。
        若为 ``None``，则 ``[2]`` 由 :attr:`targets` 列表替换。
        """
        ...

    @property
    def delay0(self) -> int:
        """动画延迟 0（毫秒，i32），供 Web 端使用。"""
        ...

    @property
    def delay1(self) -> int:
        """动画延迟 1（毫秒，i32），供 Web 端使用。"""
        ...

    @property
    def message(self) -> str:
        """原始消息模板字符串，可能含 ``[0]``/``[1]``/``[2]`` 占位符。"""
        ...

    @property
    def caster_id(self) -> int:
        """施法者玩家 ID（PlrId / usize），对应模板中的 ``[0]``。"""
        ...

    @property
    def target_id(self) -> int:
        """目标玩家 ID（PlrId / usize），对应模板中的 ``[1]``。"""
        ...

    @property
    def targets(self) -> list[int]:
        """
        多目标玩家 ID 列表（list[usize]），对应模板中的 ``[2]``（与 :attr:`param` 二选一）。
        若 :attr:`param` 不为 ``None``，本字段通常为空列表。
        """
        ...

    def target_is_empty(self) -> bool:
        """返回 :attr:`targets` 列表是否为空。"""
        ...

    def get_update_type(self) -> str:
        """
        返回事件类型的调试字符串，对应 Rust ``UpdateType`` 的 ``Debug`` 格式。

        可能的值：``"Win"``、``"None"``、``"NextLine"``。
        """
        ...

    def is_win(self) -> bool:
        """是否为胜利帧（UpdateType::Win）。"""
        ...

    def is_none(self) -> bool:
        """是否为空占位帧（UpdateType::None）。"""
        ...

    def is_next_line(self) -> bool:
        """是否为换行帧（UpdateType::NextLine）。"""
        ...

    def msg(self) -> str:
        """
        将 :attr:`message` 中的占位符替换后，返回最终可显示字符串。

        替换规则：

        * ``[0]`` → :attr:`caster_id` 的字符串形式
        * ``[1]`` → :attr:`target_id` 的字符串形式
        * ``[2]`` → :attr:`param`（若有），否则为 :attr:`targets` 的逗号拼接
        """
        ...


# ---------------------------------------------------------------------------
# Player — 玩家信息接口
# ---------------------------------------------------------------------------

class Player:
    """
    玩家信息接口，供引擎访问玩家相关数据。
    """

    @property
    def id(self) -> int:
        """玩家 ID（PlrId / u64）。"""
        ...

    @property
    def ptr(self) -> int:
        """玩家运行期指针 ID（PlrId / usize）。"""
        ...

    @property
    def name_factor(self) -> float:
        """玩家名字因子（f64）。"""
        ...

    @property
    def id_name(self) -> str:
        """玩家 ID 名字字符串。"""
        ...

    @property
    def id_key_name(self) -> str:
        """玩家 ID 键名字符串。"""
        ...

    @property
    def display_name(self) -> str:
        """玩家显示名字字符串。"""
        ...

    @property
    def clan_name(self) -> str:
        """玩家氏族名字字符串。"""
        ...

    @property
    def base_name(self) -> str:
        """玩家基础名字字符串。"""
        ...

    @property
    def weapon_name(self) -> str | None:
        """玩家武器名（若无则为 None）。"""
        ...

    @property
    def player_type(self) -> str:
        """玩家类型（Rust `PlayerType` 的 Debug 字符串）。"""
        ...

    @property
    def sort_int(self) -> int:
        """排序用随机值。"""
        ...

    @property
    def move_point(self) -> int:
        """当前移动点数。"""
        ...

    @property
    def mp(self) -> int:
        """当前 MP。"""
        ...

    @property
    def hp(self) -> int:
        """当前 HP。"""
        ...

    @property
    def max_hp(self) -> int:
        """最大 HP。"""
        ...

    @property
    def attack(self) -> int:
        ...

    @property
    def defense(self) -> int:
        ...

    @property
    def speed(self) -> int:
        ...

    @property
    def agility(self) -> int:
        ...

    @property
    def magic(self) -> int:
        ...

    @property
    def resistance(self) -> int:
        ...

    @property
    def wisdom(self) -> int:
        ...

    @property
    def point(self) -> int:
        ...

    @property
    def frozen(self) -> bool:
        ...

    @property
    def at_boost(self) -> float:
        ...

    @property
    def attract(self) -> float:
        ...

    @property
    def attr_sum(self) -> int:
        ...

    @property
    def atk_sum(self) -> int:
        ...

    @property
    def all_sum(self) -> int:
        ...

    @property
    def negative_state_count(self) -> int:
        ...

    def active(self) -> bool:
        ...

    def alive(self) -> bool:
        ...

    def check_move(self) -> bool:
        ...

    def __str__(self) -> str:
        """返回玩家的字符串表示形式。"""
        ...


# ---------------------------------------------------------------------------
# RunUpdates — 单回合事件帧集合
# ---------------------------------------------------------------------------

class RunUpdates:
    """
    单回合内所有战斗事件帧的容器。

    可从 Python 直接创建，然后传给 :meth:`Runner.round_tick` 反复复用；
    也可通过 :meth:`Runner.round_tick_new_update` 获取引擎新建的实例。

    注意
    ----
    当前绑定层未直接暴露内部 ``updates`` 列表的迭代接口；
    如需逐帧访问，请使用 :meth:`Runner.round_tick_new_update` 配合
    引擎侧扩展后续版本中的迭代 API。
    """

    def __init__(self) -> None:
        """创建一个空的 RunUpdates 容器。"""
        ...

    @staticmethod
    def new_no_capture() -> RunUpdates:
        """
        创建一个不采集详细事件帧的容器。

        适合高性能统计场景：仍可通过 :meth:`had_updates` 判断本轮是否有活动，
        但 ``updates`` 列表不会累积具体文本帧。
        """
        ...

    def clear(self) -> None:
        """清空本批次内所有事件帧及回调列表，以便复用。"""
        ...

    def reset(self) -> None:
        """同 :meth:`clear`，并重置内部活动标记。"""
        ...

    @property
    def id(self) -> int:
        """返回此批次更新的唯一标识符（u64）。"""
        ...

    @property
    def capture_updates(self) -> bool:
        """当前是否采集详细更新帧。"""
        ...

    @property
    def updates(self) -> list[RunUpdate]:
        """
        返回此批次内所有事件帧的列表。
        解决了 RunUpdates -> RunUpdate 滚木的问题

        Returns
        -------
        list[RunUpdate]
            本批次收集到的所有战斗事件帧。
        """
        ...

    @property
    def on_update_end(self) -> list[int]:
        """本批次结束后触发 on_update_end 的玩家 ID 列表。"""
        ...

    def len(self) -> int:
        """当前 updates 帧数量。"""
        ...

    def is_empty(self) -> bool:
        """当前 updates 是否为空。"""
        ...

    def had_updates(self) -> bool:
        """本批次是否发生过有效事件（不依赖是否采集详细帧）。"""
        ...


# ---------------------------------------------------------------------------
# RC4 — 核心算法
# ---------------------------------------------------------------------------

@final
class RC4:
    """
    RC4 核心算法接口，供引擎访问核心算法功能。
    """

    def __init__(self, keys: bytes, round: int | None = 1) -> None:
        """使用 key 初始化 RC4。`keys` 不能为空。"""
        ...

    @staticmethod
    def val_len() -> int:
        """返回 RC4 S 盒长度（固定 256）。"""
        ...

    @property
    def i(self) -> int:
        """RC4 状态变量 i（u32）。"""
        ...

    @i.setter
    def i(self, val: int) -> None:
        """设置 RC4 状态变量 i（u32）。"""
        ...

    @property
    def j(self) -> int:
        """RC4 状态变量 j（u32）。"""
        ...

    @j.setter
    def j(self, val: int) -> None:
        """设置 RC4 状态变量 j（u32）。"""
        ...

    def get_val(self) -> bytes:
        """获取当前 S 盒完整字节数组。"""
        ...

    def get_val_at(self, index: int) -> int:
        """读取 S 盒指定下标字节。"""
        ...

    def set_val_at(self, index: int, value: int) -> None:
        """设置 S 盒指定下标字节。"""
        ...

    def update(self, keys: bytes, round: int | None = 1) -> None:
        """按 key 再混洗一次当前 S 盒。`keys` 不能为空。"""
        ...

    def xor_bytes(self, data: bytes) -> bytes:
        ...

    def js_xor_bytes(self, data: bytes) -> bytes:
        ...

    def xor_str(self, text: str) -> None:
        ...

    def js_xor_str(self, text: str) -> None:
        ...

    def encrypt_bytes(self, data: bytes) -> bytes:
        ...

    def encrypt_bytes_no_change(self, text: str) -> None:
        ...

    def decrypt_bytes(self, data: bytes) -> bytes:
        ...

    def next_u8(self) -> int:
        ...

    def next_i32(self, max: int) -> int:
        ...

    def round(self, keys: bytes, round: int | None = 1) -> None:
        """重置 i/j 后按 key 执行 round 混洗。`keys` 不能为空。"""
        ...

    def peek_next_u8(self) -> int:
        """查看下一个随机字节，不推进状态。"""
        ...

    def c94(self) -> bool:
        ...

    def c75(self) -> bool:
        ...

    def c50(self) -> bool:
        ...

    def c25(self) -> bool:
        ...

    def c12(self) -> bool:
        ...

    def c33(self) -> bool:
        ...

    def c66(self) -> bool:
        ...

    def rFFFFFF(self) -> int:
        ...

    def rFFFF(self) -> int:
        ...

    def r256(self) -> int:
        ...

    def r64(self) -> int:
        ...

    def r16(self) -> int:
        ...

    def r255(self) -> int:
        ...

    def r127(self) -> int:
        ...

    def r63(self) -> int:
        ...

    def r31(self) -> int:
        ...

    def r15(self) -> int:
        ...

    def r7(self) -> int:
        ...

    def r3(self) -> int:
        ...

    def r3x3(self) -> int:
        ...

# ---------------------------------------------------------------------------
# Storage — 玩家 技能 等数据存储接口
# ---------------------------------------------------------------------------

@final
class Storage:
    """
    存储接口，供引擎访问玩家、技能等数据。
    """

    def get_player_by_id(self, plr_id: int) -> Player | None:
        """根据玩家 ID 获取玩家对象。"""
        ...

    def get_player_or_pending_by_id(self, plr_id: int) -> Player | None:
        """优先从 players 查询，失败时从 pending_spawns 查询。"""
        ...

    def get_pending_spawn_player_by_id(self, plr_id: int) -> Player | None:
        """仅从 pending_spawns 按 ID 查询玩家。"""
        ...

    def get_group(self, group_id: int) -> list[int] | None:
        ...

    def group_containing(self, actor: int) -> list[int] | None:
        ...

    def group_index_of(self, actor: int) -> int | None:
        ...

    def alive_group_containing(self, actor: int) -> list[int] | None:
        ...

    def alive_group_at_team_of(self, actor: int) -> list[int] | None:
        ...

    def all_alive_ids(self) -> list[int]:
        ...

    def all_player_ids(self) -> list[int]:
        ...

    @property
    def pending_spawn_count(self) -> int:
        ...

    def pending_spawn_count_for_owner(self, owner: int) -> int:
        ...

    def pending_spawn_ids_for_owner(self, owner: int) -> list[int]:
        ...

    def pending_spawn_ids_for_group(self, group_members: list[int]) -> list[int]:
        ...

    @property
    def alive_group_count(self) -> int:
        ...

    @property
    def needs_sync(self) -> bool:
        ...

    @property
    def current_plr_id(self) -> int:
        """返回当前玩家 ID 计数器值。"""
        ...

# ---------------------------------------------------------------------------
# WorldState — 战斗全局状态接口
# ---------------------------------------------------------------------------

@final
class WorldState:
    """
    世界状态接口，供引擎访问当前战斗的全局状态。
    """
    @property
    def round_pos(self) -> int:
        """当前轮次指针"""
        ...

    @property
    def players(self) -> list[int]:
        """当前行动顺序玩家列表（仅存活）。"""
        ...

    @property
    def winner(self) -> list[int] | None:
        """胜者 roster（若有）。"""
        ...

    def have_winner(self) -> bool:
        """当前是否已有胜者"""
        ...

    def all_plrs(self) -> list[int]:
        ...

    def all_plr_len(self) -> int:
        ...

    def roster_count(self) -> int:
        ...

    def team_index_of(self, actor: int) -> int | None:
        ...

    def team_roster(self, team_idx: int) -> list[int] | None:
        ...

    def team_alive(self, team_idx: int) -> list[int] | None:
        ...

    def contains_alive(self, plr_id: int) -> bool:
        ...

    def winner_roster(self, team_idx: int) -> list[int] | None:
        ...

# ---------------------------------------------------------------------------
# Runner — 战斗引擎入口
# ---------------------------------------------------------------------------

@final
class Runner:
    """
    战斗引擎的唯一对外入口。

    典型用法
    --------
    .. code-block:: python

        import tswn_py

        runner = tswn_py.Runner.new_from_namerena_raw("玩家甲\\n\\n玩家乙")

        # 一次性跑完
        runner.run_to_completion()

        # 或逐 tick 推进并复用容器：
        runner2 = tswn_py.Runner.new_from_namerena_raw("玩家甲\\n\\n玩家乙")
        updates = tswn_py.RunUpdates()
        for _ in range(10000):
            runner2.round_tick(updates)
            updates.clear()
    """

    @staticmethod
    def new_from_namerena_raw(raw_str: str) -> Runner:
        """
        从名竞原始输入字符串构建 Runner。

        输入格式
        --------
        每位玩家占一行，同队玩家之间用空行分隔。
        支持 ``\\r\\n`` / ``\\r`` / ``\\n`` 换行，末尾空白自动去除。

        Parameters
        ----------
        raw_str:
            名竞原始输入字符串。

        Returns
        -------
        Runner
            初始化完毕、可立即推进的 Runner 实例。

        Raises
        ------
        RunnerError
            输入解析失败（如名字格式非法、玩家数量异常等）时抛出。
        """
        ...

    @staticmethod
    def new_from_groups_with_seed(groups: list[list[str]], seed: list[str]) -> Runner:
        """从已分组输入与 seed 列表构建 Runner。"""
        ...

    @staticmethod
    def split_namerena_into_groups(raw_str: str) -> tuple[list[list[str]], list[str]]:
        """将名竞原始输入拆分为 (分组, seed 列表)。"""
        ...

    def main_round(self) -> RunUpdates:
        """执行一个主回合并返回更新。"""
        ...

    def round_tick(self, update: RunUpdates) -> None:
        """
        执行引擎的一个 tick，将产生的事件帧追加到 ``update`` 中。

        Parameters
        ----------
        update:
            事件容器，新帧将被追加进去（不自动清空，需调用方手动
            :meth:`RunUpdates.clear`）。
        """
        ...

    def round_tick_new_update(self) -> RunUpdates:
        """
        执行引擎的一个 tick，返回本次 tick 产生的事件帧容器（新建）。

        Returns
        -------
        RunUpdates
            仅包含本次 tick 产生的事件帧的新容器。
        """
        ...

    def round_tick_new_update_no_capture(self) -> RunUpdates:
        """
        执行一个 tick，返回不采集详细帧的更新容器（仅活动标记 + 回调队列）。
        """
        ...

    def run_to_completion(self) -> bool:
        """
        一次性将整场战斗跑至结束（有胜者或达到空转上限），不收集中间帧。

        适合用于高速胜率统计等场景，避免逐帧收集的开销。

        Returns
        -------
        bool
            ``True`` 表示正常分出胜负；``False`` 表示达到 idle/轮数上限仍无胜者。
        """
        ...

    @property
    def storage(self) -> Storage:
        """返回引擎使用的数据存储接口实例。"""
        ...

    @property
    def rc4(self) -> RC4:
        """返回引擎使用的 RC4 算法接口实例。"""
        ...

    @property
    def world_state(self) -> WorldState:
        """返回引擎使用的世界状态接口实例。"""
        ...

    def have_winner(self) -> bool:
        """当前是否已有胜者"""
        ...

    def alives_flat(self) -> list[int]:
        """存活玩家扁平列表。"""
        ...

    def alives(self) -> list[list[int]]:
        """存活玩家按组列表。"""
        ...

    def all_plrs(self) -> list[int]:
        """全部玩家 ID（含死亡）。"""
        ...

    def all_plr_len(self) -> int:
        """全部玩家数量（含死亡）。"""
        ...
