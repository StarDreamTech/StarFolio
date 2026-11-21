// Exercise the generated clipboard representations in a real rich-text consumer,
// using an isolated macOS pasteboard. Never read or replace the user's clipboard.
import AppKit
struct Clip: Decodable { let html, text: String }
let clip = try JSONDecoder().decode(Clip.self, from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))
let board = NSPasteboard.withUniqueName()
defer { board.releaseGlobally() }
precondition(board.setString("<html><head><meta charset='utf-8'></head><body>\(clip.html)</body></html>", forType: .html))
precondition(board.setString(clip.text, forType: .string))
let view = NSTextView(); view.isRichText = true; view.importsGraphics = true
precondition(view.readSelection(from: board))
let rich = view.attributedString(); let s = rich.string as NSString
let start = s.range(of: "第一段"), end = s.range(of: "第二段"); var count = 0
precondition(start.location != NSNotFound && end.location != NSNotFound)
rich.enumerateAttribute(.attachment, in: NSRange(location: 0,length: rich.length)) { value, range, _ in
    if let image = value as? NSTextAttachment {
        precondition(range.location >= NSMaxRange(start) && range.location < end.location)
        precondition(image.fileWrapper?.regularFileContents?.isEmpty == false || image.image != nil)
        count += 1
    }
}
precondition(count == 1 && board.string(forType: .string) == clip.text)
