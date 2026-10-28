"""配对汇总的输入完整性和统计口径测试。"""
import json
import tempfile
import unittest
from pathlib import Path
from summarize_perf_comparison import summarize


class SummaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.write('metadata.json', {'status': 'complete', 'measured_rounds': 3})
        self.write('signature.json', [{'input': 'plain__ffa4__01.txt', 'group_sizes': [1] * 4, 'errors': 0}])
        self.records = [dict(round=i, variant=v, warmup=i == 0, seconds={'plain__ffa4__01.txt': t})
                        for i in range(4) for v, t in [('a', 10), ('b', 9)]]
        self.write('records.json', self.records)

    def tearDown(self):
        self.temp.cleanup()

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value))

    def test_pair_metrics(self):
        rows, raw = summarize(self.root)
        row = next(row for row in rows if row['group'] == 'plain_ffa')
        self.assertAlmostEqual(row['latency_change_pct'], -10)
        self.assertAlmostEqual(row['throughput_gain_pct'], 100 / 9)
        self.assertEqual(row['faster_rounds'], 3)
        self.assertEqual(sum(row['warmup'] for row in raw), len(rows))

    def test_missing_duplicate_failed_rejected(self):
        for records in [self.records[:-1], self.records + [self.records[0]]]:
            self.write('records.json', records)
            with self.assertRaises(ValueError):
                summarize(self.root)
        self.write('records.json', self.records)
        self.write('metadata.json', {'status': 'failed', 'measured_rounds': 3})
        with self.assertRaises(ValueError):
            summarize(self.root)

    def test_outlier_not_removed(self):
        self.records[-1]['seconds']['plain__ffa4__01.txt'] = 100
        self.write('records.json', self.records)
        rows, raw = summarize(self.root)
        row = next(row for row in rows if row['group'] == 'overall')
        self.assertEqual(row['faster_rounds'], 2)
        self.assertTrue(any(row['latency_change_pct'] == 900 for row in raw))


if __name__ == '__main__':
    unittest.main()
