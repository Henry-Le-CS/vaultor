// TouchID prompt via macOS LocalAuthentication framework.
// Compiled by build.rs using the cc crate.
// Called synchronously — must run on a non-main thread (Tokio worker thread is fine).

#import <LocalAuthentication/LocalAuthentication.h>
#import <Foundation/Foundation.h>
#import <dispatch/dispatch.h>

// Returns 1 on success, 0 on failure or user cancel.
int vaultor_touchid_prompt(const char *reason_cstr) {
    @autoreleasepool {
        LAContext *ctx = [[LAContext alloc] init];
        NSError *error = nil;

        NSString *reason = reason_cstr
            ? [NSString stringWithUTF8String:reason_cstr]
            : @"Unlock Vaultor";

        BOOL canEval = [ctx canEvaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics
                                        error:&error];
        if (!canEval) {
            return 0;
        }

        dispatch_semaphore_t sem = dispatch_semaphore_create(0);
        __block BOOL result = NO;

        [ctx evaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics
            localizedReason:reason
                      reply:^(BOOL success, NSError *__unused err) {
            result = success;
            dispatch_semaphore_signal(sem);
        }];

        dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);
        return result ? 1 : 0;
    }
}
