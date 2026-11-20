KDIR ?= $(HOME)/ender-os-build/kernel/build

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) ARCH=x86_64 LLVM=1 modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) ARCH=x86_64 LLVM=1 clean
