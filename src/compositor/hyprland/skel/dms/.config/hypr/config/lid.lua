-- Laptop Lid Handling
-- When lid closes: lock session and suspend. When opens: resume.
--
-- `lockAndOutputsOff` locks and blanks the display, which is what closing a lid
-- wants. Plain `lock` would leave the outputs on.
--
-- The suspend is separated with `;`, not `&&`. It used to be `&&`, so if the
-- lock command failed the suspend was skipped too and closing the lid did
-- nothing at all -- the machine stayed awake with the lid shut. Suspending must
-- not depend on the shell being there to answer.
hl.bind("switch:on:Lid Switch", hl.dsp.exec_cmd("dms ipc call lock lockAndOutputsOff; systemctl suspend"), { locked = true })
hl.bind("switch:off:Lid Switch", hl.dsp.exec_cmd(""), { locked = true })
