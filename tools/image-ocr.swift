// macOS Vision keeps image recognition on the device. Print only detected text.
import Foundation
import Vision

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write(Data("Usage: swift tools/image-ocr.swift <image>\n".utf8))
    exit(2)
}
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["zh-Hans", "zh-Hant", "en-US"]
let image = URL(fileURLWithPath: CommandLine.arguments[1])
try VNImageRequestHandler(url: image).perform([request])
for line in request.results ?? [] {
    if let best = line.topCandidates(1).first { print(best.string) }
}
