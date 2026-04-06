import tswn_py


raw = """SB

LJ"""

rate = tswn_py.win_rate(raw, 1000)
print(f"win_rate={rate:.2f}%")

rows = tswn_py.group_win_rate("SB", ["LJ", "aaa"], 1000)
for name, value in rows:
    print(f"vs {name}: {value:.2f}%")
