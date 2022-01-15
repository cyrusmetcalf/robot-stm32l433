# connect to openocd tcp server
target extended-remote localhost:3333

#Set a very useful breakpoint at main entry. 
break main 

# Reset and halt target
monitor reset halt

# load program
load

# run to main and wait for the user to continue.
continue
