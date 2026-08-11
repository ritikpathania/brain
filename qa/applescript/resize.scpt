on run argv
	set x_val to (item 1 of argv) as integer
	set y_val to (item 2 of argv) as integer
	set w_val to (item 3 of argv) as integer
	set h_val to (item 4 of argv) as integer
	
	tell application "Terminal"
		activate
		if (count of windows) > 0 then
			set bounds of front window to {x_val, y_val, x_val + w_val, y_val + h_val}
		end if
	end tell
end run
