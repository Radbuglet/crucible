#import <Foundation/Foundation.h>
#include <Metal/Metal.h>
#include <Metal/MTLCaptureManager.h>

void skia_metal_set_output_url(MTLCaptureDescriptor *descriptor, char* url) {
    descriptor.outputURL = [NSURL URLWithString: [NSString stringWithUTF8String: url]];
}

bool skia_metal_start_capture_with_catch(MTLCaptureManager *manager, MTLCaptureDescriptor *descriptor) {
    @try {
        NSError *error = NULL;
        [manager startCaptureWithDescriptor: descriptor error: &error];

        if (error != NULL) {
            NSLog(@"Error: %@", error);
            return false;
        }

        return true;
    }
    @catch (NSException *exception) {
        NSLog(@"Exception: %@", exception);
        return false;
    }
}
