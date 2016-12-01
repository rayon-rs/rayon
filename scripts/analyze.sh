# Rough and dirty shell script to scrape the `log.rs` output and
# analyze what kinds of tasks have been started and stopped. Very
# useful in tracking down deadlocks.

TICKLES=$(grep Tickle $1 | wc -l)

INJECT_JOBS=$(grep InjectJobs $1 | wc -l)
echo "Injected jobs:" $(((INJECT_JOBS * 2)))

JOINS=$(grep Join $1 | wc -l)
echo "Joins:        " $JOINS

POPPED_RHS=$(grep PoppedRhs $1 | wc -l)
POPPED_JOB=$(grep PoppedJob $1 | wc -l)
POPPED_TOTAL=$((($POPPED_RHS + $POPPED_JOB)))
echo "Popped jobs:  " $POPPED_TOTAL = rhs $POPPED_RHS + other $POPPED_JOB

FOUND_WORK=$(grep FoundWork $1 | wc -l)
echo "Found work:   " $FOUND_WORK

STOLE_WORK=$(grep StoleWork $1 | wc -l)
echo "Stole work:   " $STOLE_WORK

UNINJECTED_WORK=$(grep UninjectedWork $1 | wc -l)
echo "Uninjected:   " $UNINJECTED_WORK

echo "Join balance: " $((( $JOINS - $POPPED_TOTAL - $STOLE_WORK  )))
echo "Inj. balance: " $((( $INJECT_JOBS * 2 - $UNINJECTED_WORK )))
echo "Total balance:" $((( $FOUND_WORK + $POPPED_TOTAL - $JOINS - $INJECT_JOBS * 2 )))

