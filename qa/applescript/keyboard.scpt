on run argv
	set action_type to item 1 of argv
	
	tell application "Terminal"
		activate
	end tell
	
	tell application "System Events"
		tell process "Terminal"
			set frontmost to true
		end tell
		if action_type is "type" then
			set text_to_type to item 2 of argv
			keystroke text_to_type
		else if action_type is "key" then
			set key_name to item 2 of argv
			if key_name is "return" or key_name is "enter" then
				key code 36
			else if key_name is "esc" or key_name is "escape" then
				key code 53
			else if key_name is "tab" then
				key code 48
			else if key_name is "backspace" then
				key code 51
			else if key_name is "delete" then
				key code 117
			else if key_name is "up" then
				key code 126
			else if key_name is "down" then
				key code 125
			else if key_name is "left" then
				key code 123
			else if key_name is "right" then
				key code 124
			else if key_name is "home" then
				key code 115
			else if key_name is "end" then
				key code 119
			else if key_name is "page_up" then
				key code 116
			else if key_name is "page_down" then
				key code 121
			else if key_name is "space" then
				key code 49
			end if
		else if action_type is "shortcut" then
			set combo to item 2 of argv
			if combo is "ctrl_k" then
				keystroke "k" using {control down}
			else if combo is "ctrl_l" then
				keystroke "l" using {control down}
			else if combo is "ctrl_c" then
				keystroke "c" using {control down}
			else if combo is "ctrl_d" then
				keystroke "d" using {control down}
			else if combo is "ctrl_r" then
				keystroke "r" using {control down}
			else if combo is "ctrl_z" then
				keystroke "z" using {control down}
			else if combo is "shift_tab" then
				key code 48 using {shift down}
			end if
		else if action_type is "clear_input" then
			key code 53 -- Esc to close overlays
			delay 0.1
			key code 51 using {command down} -- clear line
		end if
	end tell
end run
