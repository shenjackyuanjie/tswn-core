"""Storage 和 WorldState 类型存根。"""

from __future__ import annotations

from ._types_player import Player

class Storage:
    """引擎的数据存储只读接口。"""
    def get_player_by_id(self, plr_id: int) -> Player | None:
        """按玩家 ID 查询。"""
        ...
    def get_player_or_pending_by_id(self, plr_id: int) -> Player | None:
        """优先查已同步玩家，失败时查 pending。"""
        ...
    def get_pending_spawn_player_by_id(self, plr_id: int) -> Player | None:
        """仅从 pending spawn 查询。"""
        ...
    def get_group(self, group_id: int) -> list[int] | None:
        """按队伍索引返回 roster。"""
        ...
    def group_containing(self, actor: int) -> list[int] | None:
        """返回包含该玩家的 roster。"""
        ...
    def group_index_of(self, actor: int) -> int | None:
        """查询玩家所在队伍索引。"""
        ...
    def alive_group_containing(self, actor: int) -> list[int] | None:
        """返回包含该玩家的存活组。"""
        ...
    def alive_group_at_team_of(self, actor: int) -> list[int] | None:
        """按玩家所属队伍返回其存活组。"""
        ...
    def all_alive_ids(self) -> list[int]:
        """所有存活玩家 ID。"""
        ...
    def all_player_ids(self) -> list[int]:
        """所有玩家 ID，含死亡。"""
        ...
    @property
    def pending_spawn_count(self) -> int:
        """当前待同步召唤数量。"""
        ...
    def pending_spawn_count_for_owner(self, owner: int) -> int:
        """指定 owner 的待同步召唤数量。"""
        ...
    def pending_spawn_ids_for_owner(self, owner: int) -> list[int]:
        """指定 owner 的待同步召唤 ID。"""
        ...
    def pending_spawn_ids_for_group(self, group_members: list[int]) -> list[int]:
        """指定队员集合对应的待同步召唤 ID。"""
        ...
    @property
    def alive_group_count(self) -> int:
        """当前非空存活组数量。"""
        ...
    @property
    def needs_sync(self) -> bool:
        """当前是否存在待同步状态。"""
        ...
    @property
    def current_plr_id(self) -> int:
        """当前玩家 ID 计数器值。"""
        ...
    @property
    def eval_rq(self) -> float:
        """当前使用的 eval_rq。"""
        ...

class WorldState:
    """战斗的全局状态只读视图。"""
    @property
    def round_pos(self) -> int:
        """当前轮次指针。"""
        ...
    @property
    def players(self) -> list[int]:
        """当前行动顺序中的存活玩家。"""
        ...
    @property
    def winner(self) -> list[int] | None:
        """胜者 roster，若已分出胜负。"""
        ...
    def have_winner(self) -> bool:
        """是否已有赢家。"""
        ...
    def winner_team_index(self) -> int | None:
        """Return the winning world team index, if any."""
        ...
    def winner_team_indices(self) -> list[int]:
        """Return winning world team indices."""
        ...
    def all_plrs(self) -> list[int]:
        """全部玩家 ID，含死亡。"""
        ...
    def all_plr_len(self) -> int:
        """全部玩家数量，含死亡。"""
        ...
    def roster_count(self) -> int:
        """队伍数量。"""
        ...
    def team_index_of(self, actor: int) -> int | None:
        """查询玩家所属队伍下标。"""
        ...
    def team_roster(self, team_idx: int) -> list[int] | None:
        """返回队伍全员 roster。"""
        ...
    def team_alive(self, team_idx: int) -> list[int] | None:
        """返回队伍当前存活成员。"""
        ...
    def contains_alive(self, plr_id: int) -> bool:
        """查询某玩家当前是否存活。"""
        ...
    def winner_roster(self, team_idx: int) -> list[int] | None:
        """返回指定队伍的 winner roster 快照。"""
        ...
