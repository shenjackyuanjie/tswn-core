"""通过真实 Python 扩展验证战斗会话的同队强化选项。"""

import unittest

import tswn_py


RAW = "bob@red\nalice@red\nother@blue\n\nsolo@green\nseed:7@!"


def states(raw, **options):
    return tswn_py.BattleSession(raw, include_skills=True, **options).initial_states()


class TeamUpgradeTests(unittest.TestCase):
    def test_matching_group_and_default(self):
        default = states(RAW)
        enabled = states(RAW, team_upgrades=True)
        disabled = states(RAW, team_upgrades=False)
        self.assertEqual(default, enabled)
        self.assertTrue(any(enabled[i] != disabled[i] for i in (0, 1)))
        self.assertEqual(enabled[2:], disabled[2:])
        for a, b in zip(enabled, disabled):
            for key in ("id", "id_name", "id_key_name", "team_index", "input_team_index", "name_factor", "move_point"):
                self.assertEqual(a[key], b[key], key)
            self.assertEqual(a["name_factor"], b["name_factor"])
        for raw in ("bob@red\nother@blue\n\nalice@red", "bob@red\nalice@blue\n\nsolo@green"):
            self.assertEqual(states(raw, team_upgrades=True), states(raw, team_upgrades=False))

    def test_overlay_cannot_override_switch_and_remains_effective(self):
        plain = "bob@red+ol:{}\nalice@red\n\nsolo@green"
        attempted = 'bob@red+ol:{"team_upgrades":true}\nalice@red\n\nsolo@green'
        self.assertEqual(states(plain, team_upgrades=False), states(attempted, team_upgrades=False))
        self.assertNotEqual(states(attempted, team_upgrades=True), states(attempted, team_upgrades=False))
        override = 'bob@red+ol:{"team_upgrades":true,"attrs":[76,76,76,76,76,76,76,300],"skills":{"fire":42}}\nalice@red\n\nsolo@green'
        result = states(override, team_upgrades=False)[0]
        self.assertEqual(result["max_hp"], 300)
        self.assertTrue(any(skill["level"] == 42 for skill in result["skills"]))

    def test_clone_and_summon_rules_remain_unchanged_without_matching_teammates(self):
        for skill, kind in (("sklclone", "clone"), ("sklsummon", "summon")):
            with self.subTest(kind=kind):
                raw = (
                    'owner@red+ol:{"attrs":[86,86,86,86,86,86,86,500],"skills":{"%s":255}}'
                    '\n\nenemy@blue+ol:{"attrs":[40,86,40,40,40,86,40,900],"skills":{}}\nseed:0@!' % skill
                )
                enabled = tswn_py.BattleSession(raw, team_upgrades=True, max_rounds=150)
                disabled = tswn_py.BattleSession(raw, team_upgrades=False, max_rounds=150)
                self.assertEqual(enabled.initial_states(), disabled.initial_states())
                frames = list(enabled)
                self.assertEqual(frames, list(disabled))
                spawned = [state for frame in frames for state in frame["states"] if state["minion_kind"] == kind]
                self.assertTrue(spawned)
                self.assertTrue(all(state["source_id"] == 0 and state["input_team_index"] == 0 for state in spawned))
                self.assertEqual(enabled.result(), disabled.result())

    def test_replay_and_preview_are_deterministic_for_each_setting(self):
        for enabled in (True, False):
            with self.subTest(team_upgrades=enabled):
                kwargs = dict(team_upgrades=enabled, include_skills=True, max_rounds=200)
                a = tswn_py.BattleSession(RAW, **kwargs)
                b = tswn_py.BattleSession(RAW, **kwargs)
                before = a.current_states()
                self.assertEqual(a.preview_targets(0, "basic"), b.preview_targets(0, "basic"))
                self.assertEqual(a.preview_targets(0, "basic"), a.preview_targets(0, "basic"))
                self.assertEqual(before, a.current_states())
                self.assertEqual(a.initial_states(), b.initial_states())
                self.assertEqual(list(a), list(b))
                self.assertEqual(a.result(), b.result())


if __name__ == "__main__":
    unittest.main()
