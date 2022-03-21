.PHONY: alveo

# pts device path printed by socat must be set. e.g. /dev/pts/8
PTS ?= PTS_MUST_BE_SET
# build profile to build axiuartd. debug or release
PROFILE ?= debug
# profile option passed to cargo build command
PROFILE_OPT ?= $(subst release,--release,$(PROFILE))
# axiuartd path
AXIUARTD ?= target/$(PROFILE)/axiuartd
# device path to access to the register.
REG_DEVICE ?= /dev/dri/renderD128
# UART core operating frequency.
CORE_FREQ_HZ ?= 300000000
# UART core base address
UART_BASE_ADDRESS ?= 0x1402000
# GPIO base address
GPIO_BASE_ADDRESS ?= 0x1400000

$(AXIUARTD):
	cargo build $(PROFILE_OPT)

# configuration for Alveo U50 configuration
alveo: $(AXIUARTD) $(PTS)
	sudo $(AXIUARTD) --reg_device $(REG_DEVICE) --xrt --reset \
	--uart_core_frequency_hz $(CORE_FREQ_HZ) \
	--uart_base_address $(UART_BASE_ADDRESS) \
	--gpio_base_address $(GPIO_BASE_ADDRESS) \
	$(PTS)
