"""玩家相关类型存根。"""

class Player:
    """战斗中的玩家只读视图。"""
    @property
    def id(self) -> int:
        """玩家 ID。"""
        ...
    @property
    def ptr(self) -> int:
        """运行期指针 ID。"""
        ...
    @property
    def name_factor(self) -> float:
        """名字因子。"""
        ...
    @property
    def id_name(self) -> str:
        """ID 名字字符串。"""
        ...
    @property
    def id_key_name(self) -> str:
        """ID 键名字符串。"""
        ...
    @property
    def display_name(self) -> str:
        """显示名字。"""
        ...
    @property
    def clan_name(self) -> str:
        """氏族名字。"""
        ...
    @property
    def base_name(self) -> str:
        """基础名字。"""
        ...
    @property
    def weapon_name(self) -> str | None:
        """武器名字，不存在时为 `None`。"""
        ...
    @property
    def player_type(self) -> str:
        """玩家类型的调试字符串。"""
        ...
    @property
    def sort_int(self) -> int:
        """内部排序使用的随机值。"""
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
        """攻击属性。"""
        ...
    @property
    def defense(self) -> int:
        """防御属性。"""
        ...
    @property
    def speed(self) -> int:
        """速度属性。"""
        ...
    @property
    def agility(self) -> int:
        """敏捷属性。"""
        ...
    @property
    def magic(self) -> int:
        """魔力属性。"""
        ...
    @property
    def resistance(self) -> int:
        """抗性属性。"""
        ...
    @property
    def wisdom(self) -> int:
        """智力属性。"""
        ...
    @property
    def point(self) -> int:
        """点数值。"""
        ...
    @property
    def frozen(self) -> bool:
        """当前是否处于冻结状态。"""
        ...
    @property
    def at_boost(self) -> float:
        """攻击相关倍率修正。"""
        ...
    @property
    def attract(self) -> float:
        """吸引度参数。"""
        ...
    @property
    def attr_sum(self) -> int:
        """基础属性总和。"""
        ...
    @property
    def atk_sum(self) -> int:
        """攻击相关总和。"""
        ...
    @property
    def all_sum(self) -> int:
        """整体总和。"""
        ...
    @property
    def negative_state_count(self) -> int:
        """负面状态数量。"""
        ...
    def active(self) -> bool:
        """当前是否可行动。"""
        ...
    def alive(self) -> bool:
        """当前是否存活。"""
        ...
    def check_move(self) -> bool:
        """当前是否允许移动。"""
        ...
    def __str__(self) -> str:
        """返回玩家的字符串表示。"""
        ...
