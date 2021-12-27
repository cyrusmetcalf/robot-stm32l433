# connect to openocd tcp server, port 3333
target remote localhost:3333

# Reset and halt target
monitor reset halt

# load program
load

#Set a very useful breakpoint at main entry. 
break main 

# run to main and wait for the user to continue.
continue
