# Renders assets/demo.gif: run `vhs assets/demo.tape` from the repository root
# after `cargo build --release`.
Output assets/demo.gif

Set Shell "bash"
Set FontSize 22
Set Width 1000
Set Height 420
Set Padding 24
Set Theme "Builtin Dark"
Set TypingSpeed 60ms

Hide
Type "export PATH=$PWD/target/release:$PATH PS1='$ ' && unset DOZE_OPTS && clear"
Enter
Show

Type "doze 2 && echo 'silent, just like sleep'"
Sleep 300ms
Enter
Sleep 2.8s

Type "doze -d 4"
Sleep 300ms
Enter
Sleep 4.8s

Type "doze -c -t 5"
Sleep 300ms
Enter
Sleep 5.8s

Type "doze -c -i 0.1 3"
Sleep 300ms
Enter
Sleep 4s
