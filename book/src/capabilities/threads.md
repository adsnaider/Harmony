# Threads

Threads are super interesting and super simple. But first let's clarify what a
thread is.

A thread is the abstraction for execution. It hides the complexity of context
switching and preemption, enabling a logical flow of execution that is
functionally uninterrupted. Note how none of the above mentions scheduling:
this is no accident. Thread scheduling is the policy on top that defines in what
order threads should be dispatched and for how long. There are many kinds of
scheduling: Round robin, priority, CFS, preemptive, non-preemptive, etc.

## Thread Capability

The Harmony kernel provides no scheduling policy, instead it provides the
dispatch operation on thread capabilities. This is super simple: Save the current
state of the thread, and jump to the new thread.
