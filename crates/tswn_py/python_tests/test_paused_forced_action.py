"""用真实扩展验证精确暂停标记，不以状态标签代替引擎的 pre-action 决策。"""

import json
import unittest

import tswn_py


def battle_raw(seed=0, *, hide=False, counter=False):
    caster_skills = {"berserk": 128}
    target_skills = {"hide": 128} if hide else {}
    if counter:
        caster_skills["counter"] = 128
        target_skills["counter"] = 128
    caster = json.dumps({"attrs": [40, 86, 86, 86, 86, 86, 86, 900], "skills": caster_skills})
    target = json.dumps({"attrs": [40, 86, 40, 40, 40, 86, 40, 900], "skills": target_skills})
    return f"caster@red+ol:{caster}\n\nvictim@blue+ol:{target}\nseed:{seed}@!"


def replay(*, actions, hide=False, counter=False, decisions=(), passive=False):
    session = tswn_py.BattleSession(
        battle_raw(hide=hide, counter=counter),
        controlled_teams=[0, 1],
        actions=actions,
        passive_decisions=list(decisions),
        passive_decision_mode=passive,
        max_rounds=200,
    )
    return session, list(session)


def messages(frames, actor=None):
    return [
        update["message_template"]
        for frame in frames
        for update in frame["updates"]
        if actor is None or update["caster_id"] == actor
    ]


class PausedForcedActionTests(unittest.TestCase):
    def assert_read_only_boundary(self, session, expected):
        before = (
            session.current_states(), session.rounds_advanced(), session.frames_emitted(),
            session.paused_actor(), session.pending_passive(), session.peek_magic_cost(), session.result(),
        )
        for _ in range(3):
            self.assertEqual(session.paused_forced_action(), expected)
            self.assertIsNone(session.next_frame())
            self.assertEqual(before, (
                session.current_states(), session.rounds_advanced(), session.frames_emitted(),
                session.paused_actor(), session.pending_passive(), session.peek_magic_cost(), session.result(),
            ))

    def test_getter_is_read_only_before_during_and_after_berserk_pause(self):
        options = dict(controlled_teams=[0, 1], actions=[(0, "berserk", 0)], max_rounds=200)
        inspected = tswn_py.BattleSession(battle_raw(), **options)
        untouched = tswn_py.BattleSession(battle_raw(), **options)
        self.assertIsNone(inspected.paused_forced_action())
        frames = []
        while True:
            for _ in range(3):
                inspected.paused_forced_action()
            frame = inspected.next_frame()
            if frame is None:
                break
            frames.append(frame)
        self.assertEqual(frames, list(untouched))
        self.assertEqual(inspected.current_states(), untouched.current_states())
        self.assertEqual(inspected.paused_actor(), 1)
        self.assertIsNone(inspected.pending_passive())
        self.assertIsNone(inspected.result())
        self.assert_read_only_boundary(inspected, "berserk")

    def test_basic_pacing_is_consumed_as_berserk_not_normal_attack(self):
        actions = [(0, "berserk", 0)]
        paused, prefix = replay(actions=actions)
        self.assertEqual(paused.paused_forced_action(), "berserk")
        paced, frames = replay(actions=actions + [(1, "basic", 0)])
        self.assertEqual(frames[:len(prefix)], prefix)
        self.assertIn("[0]发起[狂暴攻击]", messages(frames, actor=1))
        self.assertNotIn("[0]发起攻击", messages(frames, actor=1))
        self.assertEqual(paced.paused_actor(), 0)
        self.assert_read_only_boundary(paced, None)
        # 引擎接管时，连未装备的脚本技能也仍被吞掉，不校验或支付其 MP。
        ignored, ignored_frames = replay(actions=actions + [(1, "fire", 255)])
        self.assertEqual(frames, ignored_frames)
        self.assertEqual(paced.current_states(), ignored.current_states())
        # 下一次普通攻击需要新的指令，上一条 basic 没有残留到下一回合。
        following, following_frames = replay(actions=actions + [(1, "basic", 0), (0, "basic", 0)])
        self.assertEqual(following.paused_actor(), 1)
        self.assertIsNone(following.paused_forced_action())
        self.assertEqual(messages(following_frames, actor=1).count("[0]发起[狂暴攻击]"), 1)
        self.assertNotIn("[0]发起攻击", messages(following_frames, actor=1))

    def test_hide_cancels_forced_action_even_while_status_is_berserk(self):
        session, _ = replay(actions=[(0, "berserk", 0)], hide=True)
        self.assertEqual(session.paused_actor(), 1)
        self.assertTrue(any(label.startswith("狂暴") for label in session.current_states()[1]["status_labels"]))
        self.assert_read_only_boundary(session, None)
        paced, frames = replay(actions=[(0, "berserk", 0), (1, "basic", 0)], hide=True)
        self.assertIn("[0]发起攻击", messages(frames, actor=1))
        self.assertNotIn("[0]发起[狂暴攻击]", messages(frames, actor=1))
        self.assertTrue(any(label.startswith("狂暴") for label in paced.current_states()[1]["status_labels"]))

    def test_pacing_does_not_skip_passive_boundaries(self):
        actions = [(0, "berserk", 0)]
        decisions = []
        requests = []
        # 狂暴术先等待回避和反击；即使状态已狂暴，也不能伪报主动强制暂停。
        for expected in ("evade", "core.skill.counter"):
            session, _ = replay(actions=actions, counter=True, passive=True, decisions=decisions)
            request = session.pending_passive()
            self.assertEqual(request["skill"], expected)
            self.assertEqual(request["actor"], 1)
            self.assertEqual(request["batch_id"], 1)
            self.assertIsNone(session.paused_actor())
            self.assert_read_only_boundary(session, None)
            requests.append(request)
            decisions.append((request["actor"], request["skill"], False, False, None))
        session, _ = replay(actions=actions, counter=True, passive=True, decisions=decisions)
        self.assertEqual(session.paused_actor(), 1)
        self.assert_read_only_boundary(session, "berserk")
        actions.append((1, "basic", 0))
        # 补 pacing 后必须再次停在狂暴伤害的回避/反击边界，不可自动跳过决策。
        for expected in ("evade", "core.skill.counter"):
            session, frames = replay(actions=actions, counter=True, passive=True, decisions=decisions)
            request = session.pending_passive()
            self.assertEqual(request["skill"], expected)
            self.assertEqual(request["actor"], 0)
            self.assertEqual(request["initiator"], 1)
            self.assertEqual(request["batch_id"], 2)
            self.assertIsNone(session.paused_actor())
            self.assertIn("[0]发起[狂暴攻击]", messages(frames, actor=1))
            self.assert_read_only_boundary(session, None)
            requests.append(request)
            decisions.append((request["actor"], request["skill"], False, False, None))
        session, frames = replay(actions=actions, counter=True, passive=True, decisions=decisions)
        self.assertEqual(session.paused_actor(), 0)
        self.assertIsNone(session.pending_passive())
        self.assert_read_only_boundary(session, None)
        self.assertEqual(len(requests), 4)
        self.assertIn("[1]从[狂暴]中解除", messages(frames))

    def test_ordinary_finished_truncated_and_failed_are_not_forced(self):
        ordinary, _ = replay(actions=[])
        self.assertIsNotNone(ordinary.paused_actor())
        self.assert_read_only_boundary(ordinary, None)
        for limit in (1, 2000):
            session = tswn_py.BattleSession(battle_raw(), max_rounds=limit)
            list(session)
            self.assertTrue(session.is_done())
            self.assertEqual(session.is_finished(), limit == 2000)
            self.assert_read_only_boundary(session, None)
        failed = tswn_py.BattleSession(battle_raw(), controlled_teams=[0, 1], actions=[(0, "fire", 0)])
        for _ in range(2):
            with self.assertRaises(ValueError):
                list(failed)
            self.assertTrue(failed.is_failed())
            self.assertIsNone(failed.paused_forced_action())


if __name__ == "__main__":
    unittest.main()
