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
    将玩家名字转换为 PNG 图片的 Base64 编码字符串。

    该函数使用引擎内置的字体渲染机制，生成包含玩家名字的 PNG 图片，并返回其 Base64 编码字符串。
    生成的图片尺寸和样式与引擎内使用的名字标签一致，适合在 Web 前端直接使用。

    Parameters
    ----------
    str:
        玩家名字字符串。
    """
    ...

def name_to_png_bytes(name: str) -> bytes:
    """
    将玩家名字转换为 PNG 图片的字节数据。

    该函数使用引擎内置的字体渲染机制，生成包含玩家名字的 PNG 图片，并返回其字节数据。
    生成的图片尺寸和样式与引擎内使用的名字标签一致，适合在需要直接处理图片数据的场景使用。

    Parameters
    ----------
    str:
        玩家名字字符串。
    """
    ...

# ---------------------------------------------------------------------------
# 异常
# ---------------------------------------------------------------------------

class PyRunnerError(Exception):
    """
    Runner 初始化或运行时错误。

    由 ``Runner.new_from_namerena_raw`` 在输入解析失败时抛出。
    继承自 ``Exception``，可直接 ``except PyRunnerError`` 捕获。
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

    def clear(self) -> None:
        """清空本批次内所有事件帧及回调列表，以便复用。"""
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

        while not runner.run_to_completion():
            pass  # run_to_completion 内部自行推进至结束

        # 或逐回合推进并收集事件：
        runner2 = tswn_py.Runner.new_from_namerena_raw("玩家甲\\n\\n玩家乙")
        updates = tswn_py.PyRunUpdates()
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
        PyRunnerError
            输入解析失败（如名字格式非法、玩家数量异常等）时抛出。
        """
        ...

    def round_tick(self, update: RunUpdates) -> None:
        """
        执行引擎的一个 tick，将产生的事件帧追加到 ``update`` 中。

        Parameters
        ----------
        update:
            事件容器，新帧将被追加进去（不自动清空，需调用方手动 :meth:`PyRunUpdates.clear`）。
        """
        ...

    def round_tick_new_update(self) -> RunUpdates:
        """
        执行引擎的一个 tick，返回本次 tick 产生的事件帧容器（新建）。

        Returns
        -------
        PyRunUpdates
            仅包含本次 tick 产生的事件帧的新容器。
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
