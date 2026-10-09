# GNU Make is an optional convenience layer; Python runs the same tasks directly.
PYTHON ?= python
.DEFAULT_GOAL := help
.NOTPARALLEL:

TASKS := help doctor setup check fmt fmt-check test test-rust test-python test-web \
         test-native build build-core build-web build-desktop api api-check \
         public-check verify models media raw directml source portable
.PHONY: $(TASKS)

$(TASKS):
	@$(PYTHON) tools/dev.py $@
