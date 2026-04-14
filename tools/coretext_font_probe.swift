import Foundation
import CoreGraphics
import CoreText

func describe(_ error: CFError?) -> String {
    guard let error else {
        return "(none)"
    }

    let nsError = error as Error as NSError
    let details = nsError.userInfo
        .map { "\($0.key)=\($0.value)" }
        .sorted()
        .joined(separator: ", ")

    return "domain=\(nsError.domain) code=\(nsError.code) desc=\(nsError.localizedDescription) userInfo={\(details)}"
}

func printDescriptorSummary(for descriptors: [CTFontDescriptor]) {
    print("descriptorCount=\(descriptors.count)")

    for (index, descriptor) in descriptors.prefix(5).enumerated() {
        let attributes = CTFontDescriptorCopyAttributes(descriptor) as NSDictionary
        let postScript = attributes[kCTFontNameAttribute] ?? "(no postscript name)"
        let family = attributes[kCTFontFamilyNameAttribute] ?? "(no family)"
        let format = attributes[kCTFontFormatAttribute] ?? "(no format)"
        print("descriptor[\(index)] postscript=\(postScript) family=\(family) format=\(format)")
    }
}

func probeFont(at path: String) -> Int32 {
    let url = URL(fileURLWithPath: path)
    let cfURL = url as CFURL

    print("path=\(path)")
    print("exists=\(FileManager.default.fileExists(atPath: path))")

    if let descriptors = CTFontManagerCreateFontDescriptorsFromURL(cfURL) as? [CTFontDescriptor] {
        printDescriptorSummary(for: descriptors)
    } else {
        print("descriptorCount=0")
    }

    var registrationError: Unmanaged<CFError>?
    let registered = CTFontManagerRegisterFontsForURL(cfURL, .process, &registrationError)
    let retainedError = registrationError?.takeRetainedValue()
    print("registerFontsForURL=\(registered)")
    print("registerError=\(describe(retainedError))")

    if registered {
        let unregistered = CTFontManagerUnregisterFontsForURL(cfURL, .process, nil)
        print("unregisterFontsForURL=\(unregistered)")
    }

    if let provider = CGDataProvider(url: cfURL), let font = CGFont(provider) {
        print("cgFontLoad=true")
        print("cgFont.postScriptName=\(font.postScriptName as String? ?? "(nil)")")
        print("cgFont.numberOfGlyphs=\(font.numberOfGlyphs)")
        return 0
    }

    print("cgFontLoad=false")
    return 1
}

if CommandLine.arguments.count < 2 {
    fputs("usage: swift tools/coretext_font_probe.swift <font-path>\n", stderr)
    exit(64)
}

let exitCode = probeFont(at: CommandLine.arguments[1])
exit(exitCode)
