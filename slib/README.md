
# Build
Build the `pxp-rs` project with the following commands:
```
cd pxp-rs & cargo build
cp ./target/debug/i915s.a  /opt/intel/sgxsdk/lib64/
```

# Customization
Customize the user's Enclave project as the following:

## App.cpp
Add the following snippet codes into host application file:
```
void *u_malloc(size_t size)
{
    errno = 0;
    return malloc(size);
}

void u_free(void *ptr)
{
    errno = 0;
    return free(ptr);
}

#include <sys/ioctl.h>
int ocall_pxp_ioctl(int fd, int cmd, uint64_t arg) 
{
    int ret = ioctl(fd, cmd, (void *)arg);
	return ret;
}
```

## Enclave.cpp
Import the ioctl function:
```
extern "C" int pxp_ioctl(int fd, int cmd, uint64_t arg);
```

## EDL
Add the following codes into Enclave project's `.edl` file:
```
enclave {
    untrusted {
        void *u_malloc(size_t size)propagate_errno;
        void u_free([user_check] void *ptr);
		int ocall_pxp_ioctl(int fd, int cmd, uint64_t arg);
    };

};
```

## Makefile
Add the `-li915s` Link option into Enclave project's Makefile:
```
Enclave_Link_Flags := $(MITIGATION_LDFLAGS) $(Enclave_Security_Link_Flags) \
    -Wl,--no-undefined -nostdlib -nodefaultlibs -nostartfiles -L$(SGX_TRUSTED_LIBRARY_PATH) \
	-Wl,--whole-archive -l$(Trts_Library_Name) -Wl,--no-whole-archive \
	-Wl,--start-group -lsgx_tstdc -lsgx_tcxx -l$(Crypto_Library_Name) -li915s -l$(Service_Library_Name) -Wl,--end-group \
	-Wl,-Bstatic -Wl,-Bsymbolic -Wl,--no-undefined \
	-Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
	-Wl,--defsym,__ImageBase=0 -Wl,--gc-sections   \
	-Wl,--version-script=Enclave/Enclave.lds
```
