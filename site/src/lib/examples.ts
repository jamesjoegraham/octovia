export const EXAMPLES = {
  'Pizza Order': `Ordered -> Preparing : confirm
Preparing -> OutForDelivery : ready
OutForDelivery -> Delivered : arrive`,
  'Music Player': `title: Music Player

Stopped -> Playing : play
Playing -> Paused : pause
Paused -> Playing : resume
Playing -> Stopped : stop`,
  'Pull Request': `title: Pull Request

Draft -> Open : ready for review
Open -> ChangesRequested : request changes
ChangesRequested -> Open : push update
Open -> Approved : approve
Approved -> Merged : merge
Open -> Closed : close`,
  'CI/CD Pipeline': `title: CI/CD Pipeline

Queued -> Fetching : start
Fetching -> Building : checkout ready
Building -> Testing : compiled
Testing -> Packaging : passed
Testing -> Failed : fail
Packaging -> Deploying : artifact ready
Deploying -> Live : healthy
Failed -> Queued : retry`,
  'OAuth Login': `title: OAuth 2.0 Flow

LoggedOut -> Authorizing : sign in
Authorizing -> Consent : redirect
Consent -> CodeIssued : approve
Consent -> LoggedOut : deny
CodeIssued -> ExchangingToken : callback
ExchangingToken -> LoggedIn : token granted
LoggedIn -> LoggedOut : sign out`,
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
