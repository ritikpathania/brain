on run argv
	set project_dir to item 1 of argv
	tell application "Terminal"
		activate
		do script "cd " & quoted form of project_dir & " && ./target/debug/brain ui"
	end tell
end run
