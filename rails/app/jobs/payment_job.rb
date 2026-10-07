# Integration boundary only; requires a separately supplied Rails application.
class PaymentJob < ApplicationJob
  queue_as :payments

  def perform(payment_id, provenance_id)
    raise SecurityError, 'PAYMENT_ADAPTER_UNAVAILABLE'
  end
end
