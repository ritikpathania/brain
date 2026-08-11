import Foundation
import Vision
import AppKit

guard CommandLine.arguments.count > 1 else {
    print("")
    exit(0)
}

let imagePath = CommandLine.arguments[1]
guard let image = NSImage(contentsOfFile: imagePath),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    print("")
    exit(0)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
try? handler.perform([request])

if let results = request.results {
    for observation in results {
        if let topCandidate = observation.topCandidates(1).first {
            print(topCandidate.string)
        }
    }
}
