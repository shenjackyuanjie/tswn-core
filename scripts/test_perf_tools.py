"""性能工具的纯逻辑回归测试；不编译、不启动基准二进制。"""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


def load_tool(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pgo = load_tool('build_pgo')
compare = load_tool('compare_perf_runtime')
matrix = load_tool('generate_perf_matrix')


class PerfToolsTests(unittest.TestCase):
    def training_row(self, **changes):
        return dict({'input': 'case.txt', 'start_inclusive': 1_000_000,
                     'end_exclusive': 1_010_000, 'group_sizes': [1, 1, 1, 1],
                     'total': 10_000, 'wins': 956, 'errors': 0, 'guard_exhausted': 3}, **changes)

    def parse_training(self, rows):
        return pgo.training_results('\n'.join(map(json.dumps, rows)), ['case.txt'], 1_000_000, 10_000)

    def test_training_retains_matching_guard(self):
        first = self.parse_training([self.training_row()])
        second = self.parse_training([self.training_row()])
        self.assertEqual(first, second)
        self.assertEqual(first['case.txt']['guard_exhausted'], 3)
        self.assertNotEqual(first, self.parse_training([self.training_row(guard_exhausted=2)]))

    def test_training_rejects_incomplete_or_invalid_results(self):
        bad = [[], [self.training_row(), self.training_row()]]
        changes = [{'errors': 1}, {'start_inclusive': 0}, {'end_exclusive': 1_009_999},
                   {'total': 9999}, {'wins': 10_001}, {'guard_exhausted': -1},
                   {'group_sizes': []}, {'input': 'unknown.txt'}]
        for rows in bad + [[self.training_row(**change)] for change in changes]:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                self.parse_training(rows)

    def test_source_fingerprint_tracks_edits_additions_and_removals(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = ['Cargo.toml', 'Cargo.lock', 'crates/tswn_core/Cargo.toml',
                     'crates/tswn_core/examples/perf_runtime.rs',
                     'crates/tswn_core/examples/pgo_train.rs', 'crates/tswn_core/src/lib.rs']
            for relative in paths:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('original', encoding='utf-8')
            original = pgo.source_fingerprint(root)
            self.assertEqual(original, pgo.source_fingerprint(root))
            source = root / 'crates/tswn_core/src/lib.rs'
            source.write_text('edited', encoding='utf-8')
            self.assertNotEqual(original, pgo.source_fingerprint(root))
            source.write_text('original', encoding='utf-8')
            added = source.with_name('new.rs')
            added.write_text('new', encoding='utf-8')
            self.assertNotEqual(original, pgo.source_fingerprint(root))
            added.unlink()
            self.assertEqual(original, pgo.source_fingerprint(root))
            source.unlink()
            self.assertNotEqual(original, pgo.source_fingerprint(root))

    def test_comparison_validates_settings_counters_and_elapsed(self):
        case = Path('case.txt').resolve()
        row = dict(self.training_row(), input=str(case), runs_per_matchup=10_000,
                   threads=1, mode='win-rate', timed=False, elapsed_seconds=0.25)
        signature, elapsed = compare.parse_result(json.dumps(row), case, 10_000)
        self.assertEqual(signature['guard_exhausted'], 3)
        self.assertEqual(elapsed, 0.25)
        changes = [{'errors': 1}, {'total': 9999}, {'wins': 10_001}, {'guard_exhausted': -1},
                   {'runs_per_matchup': 1}, {'threads': 2}, {'mode': 'score'}, {'timed': True},
                   {'group_sizes': [0]}, {'elapsed_seconds': 0}, {'elapsed_seconds': float('inf')},
                   {'input': str(case.with_name('wrong.txt'))}]
        for change in changes:
            with self.subTest(change=change), self.assertRaises(ValueError):
                compare.parse_result(json.dumps(dict(row, **change)), case, 10_000)
        for output in ('', json.dumps(row) + '\n' + json.dumps(row)):
            with self.subTest(output=output), self.assertRaises(ValueError):
                compare.parse_result(output, case, 10_000)

    def test_matrix_determinism_and_independent_rosters(self):
        # 合成名字池检查固定选择规则；不把随机拆分当作运行结果筛选。
        pool = [f'name-{index:03d}' for index in range(226)]
        evaluation = matrix.generated_inputs(pool, matrix.EXPANDED_SALT, 'eval')
        training = matrix.generated_inputs(pool, matrix.TRAINING_SALT, 'train')
        self.assertEqual(evaluation, matrix.generated_inputs(pool, matrix.EXPANDED_SALT, 'eval'))
        heldout = {matrix.roster_signature(row['data']) for row in evaluation}
        trained = [matrix.roster_signature(row['data']) for row in training]
        self.assertTrue(heldout.isdisjoint(trained))
        self.assertEqual(len(trained), len(set(trained)))
        self.assertEqual(matrix.roster_signature(b'a\nb\nseed:123\n'),
                         matrix.roster_signature(b'b\n\na\n'))


if __name__ == '__main__':
    unittest.main()
