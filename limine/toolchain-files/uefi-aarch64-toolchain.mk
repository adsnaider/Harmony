define DEFAULT_VAR =
    ifeq ($(origin $1),default)
        override $(1) := $(2)
    endif
    ifeq ($(origin $1),undefined)
        override $(1) := $(2)
    endif
endef

$(eval $(call DEFAULT_VAR,ADDR2LINE_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,AR_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,CC_FOR_TARGET,clang -target aarch64-elf))
$(eval $(call DEFAULT_VAR,CXX_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,CXXFILT_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,LD_FOR_TARGET,ld.lld))
$(eval $(call DEFAULT_VAR,NM_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,OBJCOPY_FOR_TARGET,llvm-objcopy))
$(eval $(call DEFAULT_VAR,OBJDUMP_FOR_TARGET,llvm-objdump))
$(eval $(call DEFAULT_VAR,RANLIB_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,READELF_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,SIZE_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,STRINGS_FOR_TARGET,))
$(eval $(call DEFAULT_VAR,STRIP_FOR_TARGET,))

$(eval $(call DEFAULT_VAR,CC_FOR_TARGET_IS_CLANG,yes))
$(eval $(call DEFAULT_VAR,LD_FOR_TARGET_HAS_NO_PIE,yes))
