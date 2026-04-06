import tswn_py


raw = """114514

1919810"""

rate = tswn_py.win_rate(raw, 1000)
print(f"win_rate={rate:.2f}%")

rows = tswn_py.group_win_rate("114514", ["1919810", "aaa"], 1000)
for name, value in rows:
    print(f"vs {name}: {value:.2f}%")
