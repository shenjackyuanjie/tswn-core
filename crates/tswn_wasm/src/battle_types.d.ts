// 规范 core DTO 形状。没有单独的 Rust 绑定 schema。
export type BattleStatus = "running" | "finished" | "truncated";
export type BattleStopReason = "winner" | "max_rounds" | "no_progress";
export type BattleReplayOptions = BattleOptions;

export interface BattleReplay {
    status: BattleStatus;
    stop_reason: BattleStopReason;
    rounds_advanced: number;
    frames_emitted: number;
    finished: boolean;
    truncated: boolean;
    initial_states: Array<BattlePlayerState>;
    frames: Array<BattleReplayFrame>;
    final_states: Array<BattlePlayerState>;
    winner_ids: Array<number>;
    winner_team_indices: Array<number>;
    state_granularity: "round";
}

export interface BattlePlayerState {
    id: number;
    team_index: number;
    input_team_index: number | null;
    owner_id: number | null;
    source_id: number | null;
    id_name: string;
    id_key_name: string;
    icon_key: string;
    display_name: string;
    display_index: number;
    base_name: string;
    player_type: string;
    minion_kind: string | null;
    icon_png_base64: string | null;
    hp: number;
    max_hp: number;
    magic_point: number;
    move_point: number;
    attack: number;
    defense: number;
    speed: number;
    agility: number;
    magic: number;
    resistance: number;
    wisdom: number;
    point: number;
    all_sum: number;
    name_factor: number;
    at_boost: number;
    attract: number;
    frozen: boolean;
    alive: boolean;
    active: boolean;
    status_labels: Array<string>;
}

export interface BattleReplayFrame {
    frame_index: number;
    round_index: number;
    finished: boolean;
    winner_ids: Array<number>;
    updates: Array<BattleUpdate>;
    rows: Array<BattleReplayRow>;
    states: Array<BattlePlayerState>;
    total_delay: number;
}

export interface BattleUpdate {
    update_type: string;
    tone: string;
    message_template: string;
    message_rendered: string;
    caster_id: number | null;
    target_id: number | null;
    target_ids: Array<number>;
    param: number | null;
    score: number;
    delay0: number;
    delay1: number;
    hp_delta: number | null;
    is_win: boolean;
    is_next_line: boolean;
}

export interface BattleReplayRow {
    indent: boolean;
    clips: Array<BattleReplayClip>;
}

export interface BattleReplayClip {
    delay: number;
    color: string;
    tone: string;
    parts: Array<BattleReplayTextPart>;
    caster_ids: Array<number>;
    target_ids: Array<number>;
    sidebar_states: Array<BattlePlayerState>;
    sidebar_previous_states: Array<BattlePlayerState>;
    winner: boolean;
}

export interface BattleReplayTextPart {
    kind: string;
    text: string;
    player_id: number | null;
    show_hp: boolean;
    hp_before: number;
    hp_after: number;
    death_effect: boolean;
    emoji: string | null;
}

export interface BattleResult {
    status: BattleStatus;
    stop_reason: BattleStopReason;
    finished: boolean;
    truncated: boolean;
    rounds_advanced: number;
    frames_emitted: number;
    winner_ids: Array<number>;
    winner_team_indices: Array<number>;
    final_states: Array<BattlePlayerState>;
}

