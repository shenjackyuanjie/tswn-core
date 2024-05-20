pub const PROFILE_START: u32 = 33554431;

pub mod runners {
    use crate::player::Player;
    use crate::rc4::RC4;

    pub struct PlayerGroup {
        players: Vec<Player>,
    }

    impl PlayerGroup {
        /// 从一个 名竞的原始输入 中创建一个 PlayerGroup
        /// 
        /// # 要求
        /// 会默认整个输入是同一个队伍的
        /// 也就是会忽略所有 \n\n 的队伍分割
        pub fn new_from_namerena_raw(raw_input: String) -> PlayerGroup {
            // 首先以 \n 分割
            let raw_input = raw_input.split("\n");
            // 然后直接 map 生成 Player
            let players: Vec<Player> = raw_input.map(|raw_name| {
                Player::new_from_namerena_raw(raw_name.to_string())
            }).collect();
            
            PlayerGroup {
                players,
            }
        }
    }

    pub struct Runner {
        /// 应该是一个 Rc4 实例类似物
        randomer: RC4,
        /// 所有玩家 (包括 boss)
        players: Vec<PlayerGroup>,
        /// 赢家
        ///
        /// 也应该是一个队伍
        winner: Option<PlayerGroup>,
    }

    impl Runner {
        /// 从一个 名竞的原始输入 中创建一个 Runner
        /// 
        /// 其实就是解析名竞的输入格式
        pub fn new_from_namerena_raw(raw_input: String) {
            // 首先以 \n\n 分割
            let mut raw_input = raw_input.split("\n\n");

        }
    }
}
