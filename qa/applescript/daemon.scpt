on run argv
	set cmd_action to item 1 of argv
	set project_dir to item 2 of argv
	
	if cmd_action is "start" then
		do shell script "cd " & quoted form of project_dir & " && ./target/debug/brain daemon start"
	else if cmd_action is "stop" then
		do shell script "cd " & quoted form of project_dir & " && ./target/debug/brain daemon stop || true"
	else if cmd_action is "kill9" then
		do shell script "pkill -9 -f brain-daemon || true"
	end if
end run
