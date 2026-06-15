export const EXAMPLES = {
  'Simple Chain': `Idle -> Active : recheck
Active -> Processing : submit
Processing -> Done : complete`,
  'Loop Back': `title: Loop Back

Idle -> Active : start
Active -> Idle : cancel
Active -> Done : finish
Done -> Idle : reset`,
  'Branching': `Draft -> Review : submit
Review -> Approved : approve
Review -> Revisions : revise
Revisions -> Draft : redraft
Revisions -> Review : resubmit
Approved -> Published : publish`,
  'Concurrent': `title: Build Pipeline

Queued -> Fetching : start
Fetching -> Building : ready
Building -> Testing : compiled
Testing -> Packaging : passed
Testing -> Failed : fail
Packaging -> Deploying : deploy
Deploying -> Live : done
Failed -> Queued : retry`,
  'Protocol Handshake': `title: TCP Style Handshake

Closed -> SynSent : open
SynSent -> SynReceived : syn+ack
SynReceived -> Established : ack
Established -> FinWait1 : close
FinWait1 -> FinWait2 : ack
FinWait2 -> TimeWait : fin
TimeWait -> Closed : timeout`,
  'Traffic Light': `title: Traffic Light

Green -> Yellow : timer
Yellow -> Red : timer
Red -> Green : timer`,
  'Order Lifecycle': `title: Order Lifecycle

Cart -> Checkout : checkout
Checkout -> PaymentPending : place order
PaymentPending -> PaymentAuthorized : authorize
PaymentPending -> PaymentDeclined : decline
PaymentDeclined -> Checkout : retry payment
PaymentAuthorized -> FraudReview : flagged
FraudReview -> PaymentAuthorized : cleared
FraudReview -> Cancelled : reject
PaymentAuthorized -> Picking : allocate
Picking -> BackOrdered : out of stock
BackOrdered -> Picking : restocked
BackOrdered -> Cancelled : abandon
Picking -> Packed : picked
Packed -> Shipped : dispatch
Shipped -> InTransit : scan
InTransit -> OutForDelivery : last mile
OutForDelivery -> Delivered : signed
OutForDelivery -> DeliveryFailed : missed
DeliveryFailed -> InTransit : reattempt
DeliveryFailed -> Returned : give up
Delivered -> ReturnRequested : customer asks
ReturnRequested -> Returned : received
Returned -> Refunded : refund
Cancelled -> Refunded : refund
Refunded -> Closed : settle
Delivered -> Closed : auto-close`,
} as const;

export type ExampleName = keyof typeof EXAMPLES;
