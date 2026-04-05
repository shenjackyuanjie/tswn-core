import tswn_py


raw = """SB

LJ
seed:33554431@!"""

groups, seed = tswn_py.Runner.split_namerena_into_groups(raw)
prepared = tswn_py.Runner.prepare_groups(groups)
runner = tswn_py.Runner.new_from_prepared_with_seed(prepared, seed)

runner.run_to_completion()

winner = runner.world_state.winner
print(f"winner={winner}")
print(f"input_groups={runner.input_groups}")
