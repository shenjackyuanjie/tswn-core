"""Canonical BattleSession DTOs, shared with Rust, WASM and C."""
from typing import Literal, TypedDict

BattleStatus = Literal["running", "finished", "truncated"]
BattleStopReason = Literal["winner", "max_rounds", "no_progress"]

class BattleReplay(TypedDict):
    status: BattleStatus
    stop_reason: BattleStopReason
    rounds_advanced: int
    frames_emitted: int
    finished: bool
    truncated: bool
    initial_states: list[BattlePlayerState]
    frames: list[BattleReplayFrame]
    final_states: list[BattlePlayerState]
    winner_ids: list[int]
    winner_team_indices: list[int]
    state_granularity: Literal["round"]

class BattlePlayerState(TypedDict):
    id: int
    team_index: int
    input_team_index: int | None
    owner_id: int | None
    source_id: int | None
    id_name: str
    id_key_name: str
    icon_key: str
    display_name: str
    display_index: int
    base_name: str
    player_type: str
    minion_kind: str | None
    icon_png_base64: str | None
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
    point: int
    all_sum: int
    name_factor: float
    at_boost: float
    attract: float
    frozen: bool
    alive: bool
    active: bool
    status_labels: list[str]

class BattleReplayFrame(TypedDict):
    frame_index: int
    round_index: int
    finished: bool
    winner_ids: list[int]
    updates: list[BattleUpdate]
    rows: list[BattleReplayRow]
    states: list[BattlePlayerState]
    total_delay: int

class BattleUpdate(TypedDict):
    update_type: str
    tone: str
    message_template: str
    message_rendered: str
    caster_id: int | None
    target_id: int | None
    target_ids: list[int]
    param: int | None
    score: int
    delay0: int
    delay1: int
    hp_delta: int | None
    is_win: bool
    is_next_line: bool

class BattleReplayRow(TypedDict):
    indent: bool
    clips: list[BattleReplayClip]

class BattleReplayClip(TypedDict):
    delay: int
    color: str
    tone: str
    parts: list[BattleReplayTextPart]
    caster_ids: list[int]
    target_ids: list[int]
    sidebar_states: list[BattlePlayerState]
    sidebar_previous_states: list[BattlePlayerState]
    winner: bool

class BattleReplayTextPart(TypedDict):
    kind: str
    text: str
    player_id: int | None
    show_hp: bool
    hp_before: int
    hp_after: int
    death_effect: bool
    emoji: str | None

class BattleResult(TypedDict):
    status: BattleStatus
    stop_reason: BattleStopReason
    finished: bool
    truncated: bool
    rounds_advanced: int
    frames_emitted: int
    winner_ids: list[int]
    winner_team_indices: list[int]
    final_states: list[BattlePlayerState]

