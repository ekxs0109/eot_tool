import CoreText
import Foundation

enum ProbeError: Error, CustomStringConvertible {
    case invalidArguments
    case missingFont(String)
    case descriptorCreationFailed(String)
    case registrationFailed(String)

    var description: String {
        switch self {
        case .invalidArguments:
            return "usage: FonttoolCoreTextProbe <font-path>"
        case .missingFont(let path):
            return "font file not found: \(path)"
        case .descriptorCreationFailed(let path):
            return "CoreText could not create font descriptors for \(path)"
        case .registrationFailed(let message):
            return message
        }
    }
}

func probeFont(at fontPath: String) throws {
    let url = URL(fileURLWithPath: fontPath)
    let fileManager = FileManager.default
    guard fileManager.fileExists(atPath: url.path) else {
        throw ProbeError.missingFont(url.path)
    }

    guard let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL),
          CFArrayGetCount(descriptors) > 0 else {
        throw ProbeError.descriptorCreationFailed(url.path)
    }

    var registrationError: Unmanaged<CFError>?
    let registered = CTFontManagerRegisterFontsForURL(
        url as CFURL,
        .process,
        &registrationError
    )

    defer {
        if registered {
            CTFontManagerUnregisterFontsForURL(url as CFURL, .process, nil)
        }
    }

    if !registered {
        let error = registrationError?.takeRetainedValue()
        let code = error.map { CFErrorGetCode($0) } ?? 0
        let description = error
            .flatMap { CFErrorCopyDescription($0) as String? }
            ?? "unknown CoreText error"
        throw ProbeError.registrationFailed(
            "CoreText registration failed for \(url.path) (code: \(code)): \(description)"
        )
    }

    print("coretext font accepted")
}

do {
    guard CommandLine.arguments.count == 2 else {
        throw ProbeError.invalidArguments
    }

    try probeFont(at: CommandLine.arguments[1])
} catch {
    fputs("\(error)\n", stderr)
    exit(1)
}
