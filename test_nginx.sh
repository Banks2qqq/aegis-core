#!/usr/bin/expect -f
set timeout 10
spawn ssh -o StrictHostKeyChecking=no root@178.236.16.101 "nginx -t && systemctl status nginx"
expect "password:"
send "C!Fbj3GdvDov\r"
expect eof
