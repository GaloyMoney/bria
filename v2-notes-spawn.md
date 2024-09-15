I've hooked up the payjoin v2 receiver into bria but I'm a bit lost (again) as to how to get the psbts it has processed and signed. I think you mentioned doing this without a job somehow but I'm a bit stuck since it seems like the only way coins get added and signed right now wis via a job executor picking up a payout_queue and creating a new batch

I've hooked up the payjoin v2 receiver into bria that has a sender and receiver communicating. I'm a bit lost (again) as to how to get the psbt a receiver has checked can be processed and signed. I think you mentioned doing this without a job somehow but I'm a bit stuck since it seems like the only way coins get added and signed right now wis via a job executor picking up a payout_queue and creating a new batch. I think I understand how these pieces work now, but not how to relate them to the payjoin flow quite yet. We could address it on a call to get an e2e payjoin working if you have a moment to do so

// TODOTODOTODO

VERY PROBABLY: spawn_process_payout_queue

ProcessPayoutQueueData may have PayjoinProposal or ProvisionalProposal to work with

Do it with the batch or else I'll have to manually decouple a bunch of things

Then, PsbtBuilderConfig should take ProvisionalProposal as input from which to "construct" (or augment) the proposal and return it.

It should be able to be spawned manually using spawn_process_payout_queue and then triggered