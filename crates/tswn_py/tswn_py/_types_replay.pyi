"""High-level replay DTO types."""

from __future__ import annotations

from typing import Literal, TypedDict

UpdateTone = Literal["damage", "recover", "status", "action", "dodge", "knockout", "win", "line", "normal"]
UpdateType = Literal["win", "none", "next_line"]
StateGranularity = Literal["tick"]

class PlayerSnapshot(TypedDict):
    id: int
    team_index: int | None
    input_team_index: int | None
    owner_id: int | None
    source_id: int | None
    display_order: int
    id_name: str
    id_key_name: str
    icon_key: str
    display_name: str
    base_name: str
    player_type: str
    minion_kind: str | None
    hp: int
    max_hp: int
    magic_point: int
    move_point: int
    attack: int
    defense: int
    speed: int
    agility: int
    magic: int
    resistance: int
    wisdom: int
    alive: bool
    active: bool
    frozen: bool

class EventDto(TypedDict):
    type: UpdateType
    update_type: UpdateType
    tone: UpdateTone
    message_template: str
    message_rendered: str
    caster_id: int | None
    target_id: int | None
    target_ids: list[int]
    param: int | None
    score: int | None
    delay0: int
    delay1: int
    is_win: bool
    is_next_line: bool

class ReplayEvent(TypedDict):
    event: EventDto
    before_states: list[PlayerSnapshot]
    after_states: list[PlayerSnapshot]
    delay_ms: int
    raw_delay_ms: int
    row_break: bool
    state_granularity: StateGranularity

class BattleReplay(TypedDict):
    initial_states: list[PlayerSnapshot]
    events: list[ReplayEvent]
    final_states: list[PlayerSnapshot]
    winner_team_index: int | None
    winner_team_indices: list[int]
    winner_ids: list[int]
    winner_names: list[str]
    state_granularity: StateGranularity
    win_delay_ms: int

class TimedEvent(TypedDict):
    event: EventDto
    delay_ms: int
    raw_delay_ms: int
    row_break: bool
