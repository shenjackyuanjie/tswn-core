"""High-level replay DTO types."""

from __future__ import annotations

from typing import Literal, TypedDict

UpdateTone = Literal["damage", "recover", "status", "action", "dodge", "knockout", "win", "line", "normal"]
ReplayTone = Literal["normal", "damage", "recover", "knockout"]
ReplayTextPartKind = Literal["text", "highlight", "player", "data"]
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

class ReplayTextPart(TypedDict):
    kind: ReplayTextPartKind
    text: str
    player_id: int | None
    show_hp: bool
    hp_before: int
    hp_after: int
    death_effect: bool
    emoji: str | None

class ReplayClip(TypedDict):
    delay: int
    text_template: str
    color: ReplayTone
    player_id: int | None
    data: str | None
    show_hp: bool
    hp_before: int
    hp_after: int
    death_effect: bool
    emoji: str | None
    parts: list[ReplayTextPart]
    caster_ids: list[int]
    target_ids: list[int]
    sidebar_states: list[PlayerSnapshot]
    sidebar_previous_states: list[PlayerSnapshot]
    winner: bool

class ReplayRow(TypedDict):
    indent: bool
    clips: list[ReplayClip]

class ReplayFrame(TypedDict):
    finished: bool
    winner_ids: list[int]
    events: list[EventDto]
    rows: list[ReplayRow]
    states: list[PlayerSnapshot]
    total_delay: int

class BattleReplay(TypedDict):
    initial_states: list[PlayerSnapshot]
    events: list[ReplayEvent]
    frames: list[ReplayFrame]
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
