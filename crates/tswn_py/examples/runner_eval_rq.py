import tswn_py


groups = [["喘际瞬爆@昀澤"], ["蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall"]]
seed = ["seed:33554431@!"]

runner = tswn_py.Runner.new_from_groups_with_seed_and_eval_rq(groups, seed, 6.0)
runner.run_to_completion()

print(f"eval_rq={runner.storage.eval_rq}")
print(f"input_groups={runner.input_groups}")
print(f"winner={runner.world_state.winner}")
