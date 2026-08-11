on run argv
	set output_path to item 1 of argv
	tell application "Terminal"
		activate
		if (count of windows) > 0 then
			set {x1, y1, x2, y2} to bounds of front window
			set w_val to x2 - x1
			set h_val to y2 - y1
			set rect_str to (x1 as string) & "," & (y1 as string) & "," & (w_val as string) & "," & (h_val as string)
			do shell script "screencapture -x -R" & rect_str & " " & quoted form of output_path
		else
			do shell script "screencapture -x " & quoted form of output_path
		end if
	end tell
end run
